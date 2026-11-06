use amplifier_acp_connection::Connection;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::{AsyncWriteExt, duplex, split};

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
