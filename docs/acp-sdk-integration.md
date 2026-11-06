# ACP SDK integration

The protocol owner's SDK handles the conversation wire protocol. Constellation
components handle the application: who the agent is, which conversation it is
answering, what it can do, and how its work appears to people.

## Implementations

| Components | Official SDK | Integration |
| --- | --- | --- |
| Runtime, Instance, Slack, Conversations, Observatory, Persona Workshop | [Rust SDK](https://github.com/agentclientprotocol/rust-sdk) 5.0.0 | The compiled `amplifier-acp-connection` crate in this repository adapts the SDK to each application's asynchronous queues and transport. |
| Teams | [TypeScript SDK](https://github.com/agentclientprotocol/typescript-sdk) 2.4.0 | `ClientSideConnection` and the SDK stream transport own messages and callback dispatch. |
| ACP Lab, Minecraft client | [Python SDK](https://github.com/agentclientprotocol/python-sdk) commit `9853d8b514d6340000bcbd01ec07808436db80cf` | `connect_to_agent`, generated models and extension methods own the protocol. A bounded application queue preserves observation ordering and surfaces collection failures. |

The selected protocol remains stable ACP v1. SDK support for draft v2 does not
silently switch an installation to that draft. Optional Constellation extensions
are negotiated separately. Instance's authenticated network transport retains its
own transport identifier; using ACP messages does not make that transport part of
the base ACP specification.

The Rust integration is a library dependency pinned to an immutable source
revision. It does not add a shared service or couple running installations.
Owners can run different component versions on different machines. Each release
includes the integration it was built and tested with.

## Why application wrappers remain

The SDK owns framing, generated protocol types, request IDs, correlation, error
responses and callback responders. Those handwritten implementations were
removed from the migrated production paths. Application state still belongs to
the application: saved sessions, credentials, permissions, work budgets,
presentation, and uncertain-delivery records are not SDK responsibilities.

The Rust wrapper connects the SDK's local execution model to Tokio applications
and bounded queues. It preserves upstream-owned IDs when acting as a transparent
proxy and waits for actual transport writes where settlement matters.

The pinned Python SDK invokes notification callbacks independently and logs their
exceptions. A public stream observer plus upstream-generated notification types
feeds an ordered, bounded application queue so a slow collector cannot silently
reorder output or lose its failure. Session replay uses an application drain
barrier. This does not add another JSON-RPC parser or request registry.

Python's published 0.12.1 package lacks the typed compaction and message-identity
support in the pinned upstream revision. The revision still reports version
0.12.1, so an installation must verify the VCS commit in `direct_url.json` as well
as the package version. Replace the pin and reassess the wrapper when an official
release supplies the required behavior. The [SDK-first contract](../contracts/protocol-sdk.v1-candidate.md)
sets the approval and removal requirements for any exception.

## What the checks establish

Component tests use real SDK peers and subprocesses to exercise initialization,
session replay, callbacks, cancellation, ordered notifications, closure, malformed
input, and negotiated extensions. A disposable Instance boundary also exercises
repeated compaction in the same session, denied effectful callbacks, matching
terminal settlement, source revisions, and session isolation. Delivery or
incorporation receipts are not task-completion receipts.

The published Runtime 0.12.0 Linux ARM64 binary was checked with upstream
[ACP TCK](https://github.com/agentclientprotocol/acp-tck) revision
`b7f40fb3c003c45926547be5a1f7e2b374035815`: 67 passed and 7 skipped, using
the trusted-local MCP profile and a deterministic local Responses fixture.
This verifies the exercised protocol behavior, not model quality, every provider,
or certification. The runtime's `tests/tck.py` records the command, capability
profile, binary, and individual results.

The platform compatibility kit should keep four results distinct: standard ACP,
negotiated optional capabilities, installation/transport compatibility, and
observable experience quality. A candidate may pass basic ACP while lacking
steering, compaction, portable identity or management controls. Report those as
absent or not exercised rather than silently adding a bridge or claiming support.
Test clients and test agents are separate roles; an agent-conformance runner is
not evidence that an experience client conforms.

Source merges, published releases, installed versions and live experience tests
are separate checkpoints. Consult [implementation status](STATUS.md) and each
component's release notes for the deployed scope. Minecraft owns its paired
body/controller deployment, including its Python environment.
