use amplifier_acp_connection::{Relay, is_message};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex, split};

#[tokio::test]
async fn relay_preserves_foreign_ids_metadata_and_flushes_before_success() {
    let (a, b) = duplex(4096);
    let (ar, aw) = split(a);
    let (br, mut bw) = split(b);
    let mut relay = Relay::new(ar, aw, 1024);
    let mut reader = BufReader::new(br);
    let request = json!({"jsonrpc":"2.0","id":"external:7","method":"_session/steering",
        "params":{"activePromptId":42,"_meta":{"unknown":{"v":1}}}});
    relay.sender.send(request.clone()).await.unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), request);
    let answer = json!({"jsonrpc":"2.0","id":"external:7","result":{"_meta":{"unknown":[1,2]}}});
    bw.write_all(format!("{answer}\n").as_bytes())
        .await
        .unwrap();
    assert_eq!(relay.recv().await.unwrap().unwrap(), answer);
}

#[tokio::test]
async fn relay_bounds_input_and_never_accepts_invalid_envelopes() {
    assert!(!is_message(
        &json!({"jsonrpc":"2.0","id":1,"error":{"code":1}})
    ));
    let (a, mut b) = duplex(4096);
    let (ar, aw) = split(a);
    let mut relay = Relay::new(ar, aw, 128);
    b.write_all(&[b'x'; 129]).await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), relay.recv())
        .await
        .unwrap();
    assert!(
        result.is_err() || matches!(result, Ok(None)),
        "Oversized input must close, never be delivered"
    );
}

#[tokio::test]
async fn relay_does_not_report_a_blocked_write_as_delivered() {
    let (a, _b) = duplex(32);
    let (ar, aw) = split(a);
    let relay = Relay::new(ar, aw, 1024);
    let result = tokio::time::timeout(
        Duration::from_millis(50),
        relay.sender.send(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}),
        ),
    )
    .await;
    assert!(result.is_err(), "Enqueue alone is not a transport flush");
}

#[tokio::test]
async fn message_channel_client_uses_sdk_ids_and_callback_dispatch() {
    let (to_client, incoming) = tokio::sync::mpsc::channel(8);
    let (outgoing, mut from_client) = tokio::sync::mpsc::channel(8);
    let mut client = amplifier_acp_connection::Connection::client_messages(incoming, outgoing)
        .await
        .unwrap();
    let id = client.sender.request("_test/request", json!({})).unwrap();
    assert_eq!(from_client.recv().await.unwrap()["id"], id);
    to_client
        .send(json!({"jsonrpc":"2.0","id":"permission","method":"_test/callback","params":{}}))
        .await
        .unwrap();
    assert_eq!(client.recv().await.unwrap().unwrap()["id"], "permission");
    client
        .sender
        .send(json!({"id":"permission","result":{}}))
        .unwrap();
    assert_eq!(from_client.recv().await.unwrap()["result"], json!({}));
    to_client
        .send(json!({"jsonrpc":"2.0","id":id,"result":{"done":true}}))
        .await
        .unwrap();
    assert_eq!(
        client.recv().await.unwrap().unwrap()["result"]["done"],
        true
    );
}
