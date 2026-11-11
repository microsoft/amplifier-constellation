# ACP integration boundaries

The protocol owner's SDK handles protocol types, framing, request correlation,
callbacks and connection lifecycle. Constellation services own application
meaning: identity, source and audience, work authority, context and delivery.
The [SDK agreement](../contracts/protocol-sdk.v1-candidate.md) governs this split.

## Reuse the protocol integration

Use the official [Rust](https://github.com/agentclientprotocol/rust-sdk),
[TypeScript](https://github.com/agentclientprotocol/typescript-sdk) or
[Python](https://github.com/agentclientprotocol/python-sdk) SDK appropriate to the
component. Read the component's dependency manifest and lock for its actual pin;
this guide does not maintain a second dependency-version table.

The [amplifier-acp-connection library](../crates/acp-connection/README.md) adapts
the Rust SDK to application queues and transport and includes shared speech
assembly. It is a compiled dependency, not a required shared service. An owner
can use a different independently released component while honoring the same
negotiated boundary.

Stable ACP v1 and optional capabilities are separate claims. Instance's
custom authenticated network hop keeps its own transport identity; ordinary
stdio attach remains the compatibility entry point. Do not infer that a remote
URL is a standard ACP transport or enable draft behavior merely because an SDK
can represent it.

## Keep wrappers narrow

An application may need ordered observation queues, transport adapters or a
settlement barrier. These preserve application behavior around the SDK, rather
than replacing its JSON-RPC machinery. Preserve upstream request IDs and unknown
metadata when relaying transparently. Durable source records, permission policy
and uncertain outcomes still belong to their application owners.

Document a concrete upstream gap, the smallest augmentation, its checks and the
condition for removing it. An immutable official upstream revision can be used
when required support is not yet published; verify the actual installed revision
rather than relying on an unchanged package version. Reassess exceptions when
the upstream mechanism becomes sufficient.

## Evaluate the connection you are changing

Exercise actual SDK peers, callbacks, ordered notifications, closure, cancellation,
malformed input, reconnect and the capabilities the consumer uses. Use the
[official ACP TCK](https://github.com/agentclientprotocol/acp-tck) for its applicable
agent checks, alongside client and integration checks.

Keep four questions distinct:

1. Does this exact agent satisfy the exercised standard ACP cases?
2. Do both sides support the optional capability being used?
3. Does the actual installation and transport preserve the behavior?
4. Does the combined experience produce the intended useful outcome?

Record the exact artifacts and conditions with the check that needs them. An
agent-conformance run does not prove that an experience client conforms; a local
provider fixture does not prove live model quality. Neither old release counts
nor a guide's wording can replace a current check of the relevant boundary.
