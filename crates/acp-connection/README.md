# ACP SDK connection integration

This small build-time library connects Tokio-based applications to the
[official ACP Rust SDK](https://github.com/agentclientprotocol/rust-sdk), pinned
to 5.0.0. Each application includes the library in its own executable. There is
no shared running service and no agent policy in this library.

`Connection` uses the SDK's ACP v1 endpoint, request IDs, response correlation,
responders, callback cancellation, and ordered dispatch. Applications receive
bounded queues of notifications, callback requests, and terminal responses.
The SDK's untyped message API keeps negotiated extensions and their metadata
intact; each application still validates the methods and capabilities it offers.

`Relay` uses the SDK's `Lines`, `Channel`, and JSON-RPC envelope types for a
transparent bridge. It preserves the request IDs belonging to the endpoints on
either side, including active-prompt bindings. It does not allocate IDs or keep
an RPC pending-response table. Instance's authenticated WebSocket transport is
still an explicitly custom transport, not standard ACP networking.

## Narrow application wrappers

The wrapper adds Tokio stream adaptation, per-frame limits, a bounded event
queue, and process-independent lifetime handling. The SDK does not supply these
application-specific resource policies. The incoming responder map retains SDK
responders until the application answers its callbacks; it does not correlate
outgoing RPC responses.

Queue overflow ends the connection with an observable error after accepted
events, even when the queue is too full to carry an error notification. A relay
send completes only after the SDK transport flushes the frame. Cancellation
after enqueue is uncertain delivery, not permission to replay it. A terminal
response still establishes only the outcome reported by that endpoint, not
completion of external work.

For terminal byte-stream cleanup, `Connection::close_and_drain()` seals all
`Sender` clones, lets the SDK flush accepted output and shut down the writer,
then discards remaining input until EOF. Consumers stop application handling
when they begin closing. Queued or in-flight SDK callbacks can finish before
foreground shutdown is observed, and accepted events can remain readable after
cancelling the wait. Further application delivery, response correlation or
session metadata handling is not promised. Already cancelled requests remain
subject to SDK cancellation. No new
protocol messages or retirement acknowledgment are introduced.

Cancelling this wait leaves shutdown in progress; another call can await it.
Dropping the connection still aborts immediately. The application owns the finite
deadline, child-process disposal and actual child wait; input EOF is not process
exit or proof that auxiliary work completed. Message-backed connections reject
terminal close without changing their existing channel behavior.

Revisit these wrappers when updating the upstream SDK. Replace stream adapters,
queues, or flush tracking when upstream offers the equivalent integration.
Keep protocol framing, ID allocation, correlation, cancellation, and dispatch
upstream; do not add a second protocol implementation here. See the
[SDK contract](../../contracts/engine-compatibility.md).

## Validation

Run `cargo test --locked` in this directory. Tests exercise ordered updates and
terminal responses, overlapping callbacks, SDK cancellation, input limits,
closure and queue overflow, extension metadata, foreign IDs, and transport
flush. Component tests exercise application policy separately. These tests are
compatibility evidence, not protocol certification.

Consumers pin an immutable commit from this repository in `Cargo.toml` and
commit `Cargo.lock`. A sibling checkout path is for local development only and
must not be published as a component dependency.
