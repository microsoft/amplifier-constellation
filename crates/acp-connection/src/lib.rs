//! Application queues around the official ACP SDK. The SDK owns JSON-RPC
//! framing, IDs, response correlation, cancellation and ordered dispatch.
//! This crate adds bounded Tokio I/O and exposes negotiated extension payloads
//! without discarding metadata. It does not supply agent policy or persistence.

use agent_client_protocol as acp;
use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::Poll,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

mod relay;
pub mod speech;
pub use relay::{Relay, RelaySender, is_message, parse_message};

type Responders = Arc<Mutex<HashMap<String, acp::Responder<Value>>>>;
type Events = mpsc::Sender<Result<Value>>;
type Drain = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

#[derive(Clone)]
enum Wire {
    Client(acp::v1::ConnectionTo<acp::Agent>),
    Agent(acp::v1::ConnectionTo<acp::Client>),
}

/// Sending half of a connection. Clones share the same SDK connection.
#[derive(Clone)]
pub struct Sender {
    wire: Wire,
    responders: Responders,
    events: Events,
    closing: Arc<Mutex<bool>>,
}

/// Receiving half and lifetime of an SDK connection.
pub struct Connection {
    pub sender: Sender,
    events: mpsc::Receiver<Result<Value>>,
    driver: JoinHandle<Result<()>>,
    bridge: Option<JoinHandle<()>>,
    ended: bool,
    close: Option<oneshot::Sender<()>>,
    close_result: Option<std::result::Result<(), String>>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.driver.abort();
        if let Some(bridge) = &self.bridge {
            bridge.abort();
        }
    }
}

impl Connection {
    /// End protocol dispatch, flush accepted SDK output and shut down the writer,
    /// then discard remaining input until EOF. This is terminal cleanup, not a
    /// half-close that continues delivering callbacks, responses or metadata.
    /// Consumers stop application handling when they begin closing: queued or
    /// in-flight SDK callbacks can finish before foreground shutdown is observed,
    /// and previously accepted events may remain readable after cancelling this
    /// wait. Further application delivery is not promised.
    /// Already cancelled SDK requests are not revived by closing.
    ///
    /// All Sender clones refuse new sends once this future is first polled.
    /// Cancelling the wait leaves shutdown running; another call can await it.
    /// Drop still aborts immediately. The caller owns any deadline and process
    /// cleanup; EOF does not establish child exit or completion of external work.
    /// Message-backed connections return an error without changing their state.
    pub async fn close_and_drain(&mut self) -> Result<()> {
        if let Some(result) = &self.close_result {
            return result.clone().map_err(anyhow::Error::msg);
        }
        if let Some(close) = self.close.take() {
            // Serialize sealing against every synchronous SDK enqueue, including
            // sends through clones. No guard survives an await.
            *self.sender.closing.lock().unwrap() = true;
            let _ = close.send(());
        } else if !*self.sender.closing.lock().unwrap() {
            bail!("ACP terminal close requires a byte-stream connection");
        }
        let result = if self.ended {
            Err(anyhow::anyhow!("ACP connection already ended"))
        } else {
            (&mut self.driver)
                .await
                .context("ACP connection task ended")
                .and_then(|result| result)
        };
        self.ended = true;
        self.close_result = Some(result.as_ref().map(|_| ()).map_err(ToString::to_string));
        result
    }

    /// Read application events in SDK dispatch order. Replies are enqueued in
    /// the same ordered lane as notifications, not by independent waiter tasks.
    pub async fn recv(&mut self) -> Result<Option<Value>> {
        if self.ended {
            return Ok(None);
        }
        // An error notification cannot be queued when a stalled consumer has
        // filled the queue. Observe the SDK driver's end independently, while
        // preserving already accepted event order. Sender clones may outlive
        // the driver, so channel closure alone is not a termination signal.
        let result = tokio::select! {
            biased;
            event = self.events.recv() => event.transpose(),
            _ = &mut self.driver => Err(anyhow::anyhow!("ACP connection ended")),
        };
        if result.is_err() || matches!(result, Ok(None)) {
            self.ended = true;
        }
        result
    }

    /// Connect an ACP client over an already owned byte stream.
    pub async fn client<R, W>(reader: R, writer: W, max_frame: usize) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self::start(reader, writer, max_frame, false).await
    }

    /// Serve ACP v1 over an already owned byte stream.
    pub async fn agent<R, W>(reader: R, writer: W, max_frame: usize) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self::start(reader, writer, max_frame, true).await
    }

    /// In-process policy channels still use the SDK endpoint for IDs, request
    /// correlation and callbacks. This is not a second protocol implementation.
    pub async fn client_messages(
        incoming: mpsc::Receiver<Value>,
        outgoing: mpsc::Sender<Value>,
    ) -> Result<Self> {
        Self::messages(None, incoming, outgoing, false).await
    }

    /// Terminate an observation-only ACP connection without launching a model
    /// backend. The SDK still owns its requests, responders and notifications.
    pub async fn agent_messages(
        first: Value,
        incoming: mpsc::Receiver<Value>,
        outgoing: mpsc::Sender<Value>,
    ) -> Result<Self> {
        Self::messages(Some(first), incoming, outgoing, true).await
    }

    async fn messages(
        first: Option<Value>,
        mut incoming: mpsc::Receiver<Value>,
        outgoing: mpsc::Sender<Value>,
        agent: bool,
    ) -> Result<Self> {
        let (endpoint, transport) = acp::Channel::duplex();
        let (mut rx, tx) = transport.into_parts();
        let bridge = tokio::spawn(async move {
            let receive = async move {
                if let Some(value) = first
                    && tx
                        .unbounded_send(acp::TransportFrame::parse_json(&value.to_string()))
                        .is_err()
                {
                    return;
                }
                while let Some(value) = incoming.recv().await {
                    if tx
                        .unbounded_send(acp::TransportFrame::parse_json(&value.to_string()))
                        .is_err()
                    {
                        break;
                    }
                }
            };
            let send = async move {
                while let Some(frame) = rx.next().await {
                    let acp::TransportFrame::Single(message) = frame else {
                        break;
                    };
                    let Ok(value) = serde_json::to_value(message) else {
                        break;
                    };
                    if !matches!(
                        tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            outgoing.send(value)
                        )
                        .await,
                        Ok(Ok(()))
                    ) {
                        break;
                    }
                }
            };
            tokio::select! { _ = receive => {}, _ = send => {} }
        });
        match Self::start_on(endpoint, agent, Arc::default(), None).await {
            Ok(mut connection) => {
                connection.bridge = Some(bridge);
                Ok(connection)
            }
            Err(error) => {
                bridge.abort();
                Err(error)
            }
        }
    }

    async fn start<R, W>(reader: R, writer: W, max_frame: usize, agent: bool) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let closing = Arc::new(Mutex::new(false));
        // Keep the original framed reader (including its buffered bytes) after
        // SDK foreground completion. Only this driver ever polls either view.
        let reader = Arc::new(Mutex::new(FramedRead::new(
            reader,
            LinesCodec::new_with_max_length(max_frame),
        )));
        let incoming = futures_util::stream::poll_fn({
            let reader = reader.clone();
            let closing = closing.clone();
            move |cx| {
                for _ in 0..16 {
                    let item = futures_util::ready!(reader.lock().unwrap().poll_next_unpin(cx));
                    if !*closing.lock().unwrap() || matches!(item, Some(Err(_))) {
                        return Poll::Ready(item.map(|r| r.map_err(std::io::Error::other)));
                    }
                    // During terminal close, discard input but do not deliver
                    // EOF to the SDK before its accepted output has drained.
                    // Keep reading while writes flush to avoid pipe deadlock.
                    if item.is_none() {
                        return Poll::Pending;
                    }
                }
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        });
        let outgoing = <FramedWrite<W, LinesCodec> as SinkExt<String>>::sink_map_err(
            FramedWrite::new(writer, LinesCodec::new_with_max_length(max_frame)),
            std::io::Error::other,
        )
        .with(move |line: String| async move {
            if line.len() > max_frame {
                Err(std::io::Error::other("ACP output frame too large"))
            } else {
                Ok(line)
            }
        });
        let transport = acp::Lines::new(outgoing, incoming);
        let (close, requested) = oneshot::channel();
        let drain: Drain = Box::pin(async move {
            let mut remaining =
                futures_util::stream::poll_fn(move |cx| reader.lock().unwrap().poll_next_unpin(cx));
            while let Some(line) = remaining.next().await {
                line.context("ACP input failed during terminal drain")?;
            }
            Ok(())
        });
        let mut connection =
            Self::start_on(transport, agent, closing, Some((requested, drain))).await?;
        connection.close = Some(close);
        Ok(connection)
    }

    async fn start_on<T>(
        transport: T,
        agent: bool,
        closing: Arc<Mutex<bool>>,
        shutdown: Option<(oneshot::Receiver<()>, Drain)>,
    ) -> Result<Self>
    where
        T: acp::ConnectTo<acp::Agent> + acp::ConnectTo<acp::Client>,
    {
        let (events, receiver) = mpsc::channel(256);
        let responders = Responders::default();
        let (ready_tx, ready_rx) = oneshot::channel();
        let req_events = events.clone();
        let notif_events = events.clone();
        let request_responders = responders.clone();
        let closed_events = events.clone();
        let error_events = events.clone();
        let sender_responders = responders.clone();
        let sender_events = events.clone();
        let driver = tokio::spawn(async move {
            let (requested, drain) = match shutdown {
                Some((requested, drain)) => (Some(requested), Some(drain)),
                None => (None, None),
            };
            // The two concrete SDK roles have different counterpart types.
            // Both use the same application callbacks and the SDK v1 guard.
            macro_rules! run {
                ($builder:expr, $variant:ident) => {
                    $builder
                        .on_receive_request(async move |message: acp::UntypedMessage,
                            responder: acp::v1::Responder<Value>, _cx| {
                            let id = serde_json::to_value(responder.raw_id())?;
                            let key = id.to_string();
                            {
                                let mut active = request_responders.lock().unwrap();
                                if active.len() >= 256 || active.contains_key(&key) {
                                    return responder.respond_with_error(acp::Error::invalid_request());
                                }
                                active.insert(key, responder.into_raw());
                            }
                            enqueue(&req_events, json!({"jsonrpc":"2.0","id":id,
                                "method":message.method,"params":message.params}))
                        }, acp::on_receive_request!())
                        .on_receive_notification(async move |message: acp::UntypedMessage, _cx| {
                            enqueue(&notif_events, json!({"jsonrpc":"2.0",
                                "method":message.method,"params":message.params}))
                        }, acp::on_receive_notification!())
                        .on_close(async move |_cx| {
                            let _ = closed_events.try_send(Err(anyhow::anyhow!("ACP transport closed")));
                            Err(acp::Error::internal_error())
                        })
                        .connect_with(transport, async move |wire| {
                            let sender = Sender { wire: Wire::$variant(wire),
                                responders: sender_responders, events: sender_events,
                                closing };
                            let _ = ready_tx.send(sender);
                            if let Some(requested) = requested
                                && requested.await.is_ok() {
                                return Ok(());
                            }
                            std::future::pending::<Result<(), acp::Error>>().await
                        }).await
                }
            }
            let result = if agent {
                run!(acp::Agent.v1(), Agent)
            } else {
                run!(acp::Client.v1(), Client)
            };
            if result.is_err() {
                let _ = error_events.try_send(Err(anyhow::anyhow!("ACP connection ended")));
            }
            result?;
            if let Some(drain) = drain {
                drain.await?;
            }
            Ok(())
        });
        let sender = ready_rx.await.context("ACP connection could not start")?;
        Ok(Self {
            sender,
            events: receiver,
            driver,
            bridge: None,
            ended: false,
            close: None,
            close_result: None,
        })
    }
}

fn enqueue(events: &Events, value: Value) -> Result<(), acp::Error> {
    events
        .try_send(Ok(value))
        .map_err(|_| acp::Error::internal_error())
}

impl Sender {
    fn open(&self) -> Result<MutexGuard<'_, bool>> {
        let closing = self.closing.lock().unwrap();
        if *closing {
            bail!("ACP output is closing");
        }
        Ok(closing)
    }

    /// Publish a request with an SDK-assigned ID. Terminal results arrive via
    /// Connection::recv, after preceding notifications on that connection.
    pub fn request(&self, method: &str, params: Value) -> Result<Value> {
        let _open = self.open()?;
        let message = acp::UntypedMessage::new(method, params)?;
        let prepared = match &self.wire {
            Wire::Client(c) => c.prepare_request(message),
            Wire::Agent(c) => c.prepare_request(message),
        };
        let id = serde_json::to_value(prepared.raw_id())?;
        let reply_id = id.clone();
        let events = self.events.clone();
        prepared.on_receiving_result(async move |result| {
            let response = match result {
                Ok(value) => json!({"jsonrpc":"2.0","id":reply_id,"result":value}),
                Err(error) => json!({"jsonrpc":"2.0","id":reply_id,"error":error}),
            };
            enqueue(&events, response)
        })?;
        Ok(id)
    }

    /// Await a callback using SDK cancellation-on-drop. No application waiter
    /// registry or request-ID generator is involved.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request = acp::UntypedMessage::new(method, params)?;
        let reply = {
            let _open = self.open()?;
            match &self.wire {
                Wire::Client(c) => c.send_request(request),
                Wire::Agent(c) => c.send_request(request),
            }
        };
        Ok(reply.block_task().await?)
    }

    pub fn notify(&self, method: &str, params: Value) -> Result<()> {
        let _open = self.open()?;
        let notification = acp::UntypedMessage::new(method, params)?;
        match &self.wire {
            Wire::Client(c) => c.send_notification(notification)?,
            Wire::Agent(c) => c.send_notification(notification)?,
        }
        Ok(())
    }

    /// Complete a received request through its original SDK responder, or send
    /// a notification. Requests must use request()/call(), never a supplied ID.
    pub fn send(&self, message: Value) -> Result<()> {
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            if message.get("id").is_some() {
                bail!("Use SDK request ID allocation");
            }
            return self.notify(method, message.get("params").cloned().unwrap_or(json!({})));
        }
        let _open = self.open()?;
        let id = message.get("id").context("Response has no request ID")?;
        let responder = self
            .responders
            .lock()
            .unwrap()
            .remove(&id.to_string())
            .context("Received request is no longer pending")?;
        if let Some(error) = message.get("error") {
            responder.respond_with_error(serde_json::from_value(error.clone())?)?;
        } else {
            responder.respond(message.get("result").cloned().unwrap_or(Value::Null))?;
        }
        Ok(())
    }
}
