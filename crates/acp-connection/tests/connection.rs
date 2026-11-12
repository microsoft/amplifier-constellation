use amplifier_acp_connection::Connection;
use futures_util::FutureExt;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, duplex, split};

async fn pair() -> (Connection, Connection) {
    let (a, b) = duplex(65536);
    let (ar, aw) = split(a);
    let (br, bw) = split(b);
    let (client, agent) = tokio::join!(
        Connection::client(ar, aw, 4096),
        Connection::agent(br, bw, 4096)
    );
    (client.unwrap(), agent.unwrap())
}
async fn next(c: &mut Connection) -> Value {
    tokio::time::timeout(Duration::from_secs(2), c.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn preserves_notification_response_order_and_extension_payloads() {
    let (mut client, mut agent) = pair().await;
    let id = client
        .sender
        .request(
            "_custom/request",
            json!({"_meta":{"example":{"revision":17}}}),
        )
        .unwrap();
    let request = next(&mut agent).await;
    assert_eq!(request["id"], id);
    assert_eq!(request["params"]["_meta"]["example"]["revision"], 17);
    for n in 0..20 {
        agent.sender.notify("session/update",json!({"sessionId":"s","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":n.to_string()},"_meta":{"example":{"n":n}}}})).unwrap();
    }
    agent
        .sender
        .send(json!({"id":id,"result":{"done":true}}))
        .unwrap();
    for n in 0..20 {
        assert_eq!(
            next(&mut client).await["params"]["update"]["_meta"]["example"]["n"],
            n
        );
    }
    assert_eq!(next(&mut client).await["result"]["done"], true);
}

#[tokio::test]
async fn overlapping_callbacks_and_requests_keep_their_own_ids() {
    let (mut client, mut agent) = pair().await;
    let prompt = client.sender.request("_test/prompt", json!({})).unwrap();
    assert_eq!(next(&mut agent).await["id"], prompt);
    let callback = agent.sender.request("_test/callback", json!({})).unwrap();
    assert_eq!(next(&mut client).await["id"], callback);
    client
        .sender
        .send(json!({"id":callback,"result":{"callback":true}}))
        .unwrap();
    assert_eq!(next(&mut agent).await["result"]["callback"], true);
    agent
        .sender
        .send(json!({"id":prompt,"error":{"code":-32601,"message":"Not supported"}}))
        .unwrap();
    assert_eq!(next(&mut client).await["error"]["code"], -32601);
}

#[tokio::test]
async fn dropping_callback_wait_sends_sdk_cancellation() {
    let (mut client, agent) = pair().await;
    let sender = agent.sender.clone();
    let call = tokio::spawn(async move { sender.call("_test/callback", json!({})).await });
    let request = next(&mut client).await;
    call.abort();
    let _ = call.await;
    let cancel = next(&mut client).await;
    assert_eq!(cancel["method"], "$/cancel_request");
    assert_eq!(cancel["params"]["requestId"], request["id"]);
}

#[tokio::test]
async fn oversized_input_ends_connection_and_closure_is_observable() {
    let (a, mut b) = duplex(65536);
    let (ar, aw) = split(a);
    let mut client = Connection::client(ar, aw, 128).await.unwrap();
    b.write_all(&[b'x'; 129]).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), client.recv())
            .await
            .unwrap()
            .is_err()
    );
    let (mut client, agent) = pair().await;
    drop(agent);
    assert!(
        tokio::time::timeout(Duration::from_secs(2), client.recv())
            .await
            .unwrap()
            .is_err()
    );
}

#[tokio::test]
async fn update_overflow_cannot_be_reported_as_a_successful_turn() {
    let (mut client, mut agent) = pair().await;
    let id = client.sender.request("_test/prompt", json!({})).unwrap();
    assert_eq!(next(&mut agent).await["id"], id);
    for n in 0..300 {
        let _ = agent.sender.notify("session/update", json!({"sessionId":"s",
            "update":{"sessionUpdate":"agent_message_chunk", "content":{"type":"text", "text":n.to_string()}}}));
    }
    let _ = agent.sender.send(json!({"id":id,"result":{"done":true}}));
    // Let the bounded consumer queue fill before reading, as a stalled UI can.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let outcome = tokio::time::timeout(Duration::from_secs(2), async {
        while let Ok(Some(message)) = client.recv().await {
            assert_ne!(
                message["result"]["done"], true,
                "Dropped observations cannot be followed by a successful turn"
            );
        }
    })
    .await;
    assert!(
        outcome.is_ok(),
        "Overflow must terminate rather than wait forever"
    );
}

#[tokio::test]
async fn endpoint_rejects_supplied_request_ids_and_replies_once() {
    let (mut client, mut agent) = pair().await;
    assert!(
        client
            .sender
            .send(json!({"id":"invented", "method":"_test/nope", "params":{}}))
            .is_err()
    );
    let id = client.sender.request("_test/once", json!({})).unwrap();
    assert_eq!(next(&mut agent).await["id"], id);
    agent.sender.send(json!({"id":id,"result":{}})).unwrap();
    assert!(agent.sender.send(json!({"id":id,"result":{}})).is_err());
    assert_eq!(next(&mut client).await["id"], id);
}

#[tokio::test]
async fn terminal_close_flushes_fifo_seals_clones_and_drains_reverse_output() {
    let (a, b) = duplex(8);
    let (ar, aw) = split(a);
    let (br, mut bw) = split(b);
    let mut client = Connection::client(ar, aw, 4096).await.unwrap();
    let sender = client.sender.clone();
    let peer = tokio::spawn(async move {
        bw.write_all(
            b"{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"_test/callback\",\"params\":{}}\n",
        )
        .await
        .unwrap();
        let mut lines = BufReader::new(br).lines();
        for n in 0..32 {
            let line = lines.next_line().await.unwrap().unwrap();
            let message: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(message["params"]["n"], n);
        }
        let response: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(response["id"], 7);
        assert_eq!(response["result"]["accepted"], true);
        assert!(lines.next_line().await.unwrap().is_none());
        // The peer keeps output alive after observing physical input EOF.
        // Several pipe capacities must be drained without further dispatch.
        for _ in 0..32 {
            bw.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"_test/late\",\"params\":{}}\n")
                .await
                .unwrap();
        }
        bw.shutdown().await.unwrap();
    });
    assert_eq!(next(&mut client).await["id"], 7);
    for n in 0..32 {
        sender.notify("_test/ordered", json!({"n":n})).unwrap();
    }
    sender
        .send(json!({"id":7,"result":{"accepted":true}}))
        .unwrap();
    let mut closing = Box::pin(client.close_and_drain());
    assert!(closing.as_mut().now_or_never().is_none());
    assert!(sender.notify("_test/no", json!({})).is_err());
    assert!(sender.request("_test/no", json!({})).is_err());
    assert!(sender.call("_test/no", json!({})).await.is_err());
    assert!(sender.send(json!({"id":7,"result":{}})).is_err());
    tokio::time::timeout(Duration::from_secs(2), closing)
        .await
        .unwrap()
        .unwrap();
    peer.await.unwrap();
    assert!(client.recv().await.unwrap().is_none());
    client.close_and_drain().await.unwrap();
}

#[tokio::test]
async fn cancelled_close_can_resume_and_input_eof_does_not_cut_off_flush() {
    let (a, b) = duplex(8);
    let (ar, aw) = split(a);
    let (mut br, mut bw) = split(b);
    let mut client = Connection::client(ar, aw, 4096).await.unwrap();
    client.sender.request("_test/pending", json!({})).unwrap();
    for n in 0..32 {
        client
            .sender
            .notify("_test/ordered", json!({"n":n}))
            .unwrap();
    }
    assert!(client.close_and_drain().now_or_never().is_none());
    assert!(client.sender.clone().notify("_test/no", json!({})).is_err());
    // Incoming EOF arrives while accepted writes are held by backpressure.
    bw.shutdown().await.unwrap();
    let peer = async move {
        let mut bytes = Vec::new();
        br.read_to_end(&mut bytes).await.unwrap();
        let messages: Vec<Value> = String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(messages.len(), 33);
        assert_eq!(messages[0]["method"], "_test/pending");
        for (n, message) in messages[1..].iter().enumerate() {
            assert_eq!(message["params"]["n"], n);
        }
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        let (closed, ()) = tokio::join!(client.close_and_drain(), peer);
        closed.unwrap();
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn drop_still_aborts_a_blocked_terminal_close() {
    let (a, b) = duplex(8);
    let (ar, aw) = split(a);
    let (mut br, _bw) = split(b);
    let mut client = Connection::client(ar, aw, 4096).await.unwrap();
    client
        .sender
        .notify("_test/blocked", json!({"text":"x".repeat(3000)}))
        .unwrap();
    assert!(client.close_and_drain().now_or_never().is_none());
    let mut prefix = [0; 8];
    tokio::time::timeout(Duration::from_secs(2), br.read_exact(&mut prefix))
        .await
        .unwrap()
        .unwrap();
    let sender = client.sender.clone();
    drop(client);
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), br.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert!(
        bytes.len() < 3000,
        "Drop must abort rather than finish the blocked output"
    );
    assert!(sender.notify("_test/no", json!({})).is_err());
}

#[tokio::test]
async fn message_connections_reject_close_without_sealing_channels() {
    for agent in [false, true] {
        let (incoming, rx) = tokio::sync::mpsc::channel(8);
        let (tx, mut outgoing) = tokio::sync::mpsc::channel(8);
        let mut connection = if agent {
            Connection::agent_messages(
                json!({"jsonrpc":"2.0","method":"_test/first","params":{}}),
                rx,
                tx,
            )
            .await
            .unwrap()
        } else {
            Connection::client_messages(rx, tx).await.unwrap()
        };
        if agent {
            assert_eq!(next(&mut connection).await["method"], "_test/first");
        }
        assert!(connection.close_and_drain().await.is_err());
        connection
            .sender
            .notify("_test/still_open", json!({}))
            .unwrap();
        assert_eq!(outgoing.recv().await.unwrap()["method"], "_test/still_open");
        incoming
            .send(json!({"jsonrpc":"2.0","method":"_test/incoming","params":{}}))
            .await
            .unwrap();
        assert_eq!(next(&mut connection).await["method"], "_test/incoming");
    }
}
