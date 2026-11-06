//! Transparent SDK transport for a policy bridge. Foreign request IDs remain
//! unchanged: this layer does not become a second RPC endpoint.
use agent_client_protocol as acp;
use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{Mutex, Semaphore, watch},
    task::JoinHandle,
};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

/// Validate envelopes with the SDK, retaining all params/result metadata.
pub fn is_message(value: &Value) -> bool {
    serde_json::from_value::<acp::RawJsonRpcMessage>(value.clone()).is_ok()
}

pub fn parse_message(text: &str) -> Result<Value> {
    value(acp::TransportFrame::parse_json(text))
}

fn value(frame: acp::TransportFrame) -> Result<Value> {
    match frame {
        acp::TransportFrame::Single(message) => Ok(serde_json::to_value(message)?),
        // Never include malformed peer data in an error/log.
        _ => bail!("Invalid ACP JSON-RPC message"),
    }
}

#[derive(Clone)]
pub struct RelaySender {
    tx: futures_channel::mpsc::UnboundedSender<acp::TransportFrame>,
    sequence: Arc<Mutex<u64>>,
    written: watch::Receiver<u64>,
    max_frame: usize,
}

impl RelaySender {
    /// Return only after the SDK transport flushes this frame. One serialized
    /// write at a time bounds its otherwise unbounded internal outgoing queue.
    /// Cancellation after enqueue leaves delivery uncertain; callers must not
    /// replay the message merely because this future was dropped.
    pub async fn send(&self, message: Value) -> Result<()> {
        let message: acp::RawJsonRpcMessage = serde_json::from_value(message)
            .map_err(|_| anyhow::anyhow!("Invalid ACP JSON-RPC message"))?;
        if serde_json::to_vec(&message)?.len() > self.max_frame {
            bail!("ACP output frame too large");
        }
        let mut sequence = self.sequence.lock().await;
        *sequence += 1;
        let target = *sequence;
        self.tx
            .unbounded_send(acp::TransportFrame::Single(message))
            .map_err(|_| anyhow::anyhow!("ACP output closed"))?;
        let mut written = self.written.clone();
        while *written.borrow_and_update() < target {
            written
                .changed()
                .await
                .context("ACP output closed before flush")?;
        }
        Ok(())
    }
}

/// SDK Lines/Channel transport plus bounded input and flush acknowledgements.
pub struct Relay {
    pub sender: RelaySender,
    rx: futures_channel::mpsc::UnboundedReceiver<acp::TransportFrame>,
    capacity: Arc<Semaphore>,
    driver: JoinHandle<Result<(), acp::Error>>,
    ended: bool,
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.driver.abort();
    }
}

impl Relay {
    pub fn new<R, W>(reader: R, writer: W, max_frame: usize) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let capacity = Arc::new(Semaphore::new(16));
        let read_capacity = capacity.clone();
        let incoming =
            FramedRead::new(reader, LinesCodec::new_with_max_length(max_frame)).then(move |line| {
                let capacity = read_capacity.clone();
                async move {
                    let permit = capacity
                        .acquire_owned()
                        .await
                        .map_err(std::io::Error::other)?;
                    permit.forget();
                    line.map_err(std::io::Error::other)
                }
            });
        let (written_tx, written) = watch::channel(0u64);
        let outgoing = futures_util::sink::unfold(
            (
                FramedWrite::new(writer, LinesCodec::new_with_max_length(max_frame)),
                written_tx,
            ),
            |(mut writer, written), line: String| async move {
                writer.send(line).await.map_err(std::io::Error::other)?;
                written.send_modify(|sequence| *sequence += 1);
                Ok::<_, std::io::Error>((writer, written))
            },
        );
        let (channel, driver) = acp::ConnectTo::<acp::UntypedRole>::into_channel_and_future(
            acp::Lines::new(outgoing, incoming),
        );
        let (rx, tx) = channel.into_parts();
        let driver = tokio::spawn(driver.expect("SDK Lines owns its transport"));
        Self {
            sender: RelaySender {
                tx,
                sequence: Arc::new(Mutex::new(0)),
                written,
                max_frame,
            },
            rx,
            capacity,
            driver,
            ended: false,
        }
    }

    pub async fn recv(&mut self) -> Result<Option<Value>> {
        if self.ended {
            return Ok(None);
        }
        tokio::select! {
            // Preserve accepted message order before reporting transport end.
            biased;
            frame = self.rx.next() => match frame {
                Some(frame) => { self.capacity.add_permits(1); Ok(Some(value(frame)?)) },
                None => { self.ended = true; Ok(None) },
            },
            result = &mut self.driver => {
                self.ended = true;
                result.context("ACP transport task ended")?
                    .map_err(|_| anyhow::anyhow!("ACP transport failed"))?;
                Ok(None)
            }
        }
    }
}
