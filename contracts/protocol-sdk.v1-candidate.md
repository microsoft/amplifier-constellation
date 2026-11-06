# Protocol SDK contract — v1 candidate

Accepted direction · November 6, 2026. Implementation and deployment status must
be reported separately from this contract.

## SDK first

Use the protocol owner's SDK for protocol types, wire framing, request IDs,
request/response correlation, cancellation, dispatch, and connection lifecycle.
For ACP, use the [official Rust SDK](https://github.com/agentclientprotocol/rust-sdk),
[official TypeScript SDK](https://github.com/agentclientprotocol/typescript-sdk),
and [official Python SDK](https://github.com/agentclientprotocol/python-sdk)
in the corresponding components. Apply this principle to other protocol
integrations, including Agent Host Protocol, when introduced.

A dependency alone is not adoption: production calls must traverse the SDK.
Delete superseded handwritten protocol implementations after equivalent behavior
has been verified. Keep application policy, identity, authorization, context,
attention, activity presentation, and persistent work accounting in their owning
components. An SDK does not decide those policies.

## Augment without replacing

Use SDK transport adapters, raw-message escape hatches, middleware, and extension
points before replacing any SDK machinery. A custom authenticated transport may
wrap the SDK while retaining its published transport identifier. It must not be
described as a standard ACP transport merely because the messages use ACP.
Preserve unknown extension metadata and foreign request IDs in transparent
proxies; versioned extensions remain negotiated and optional.

Pin and record the SDK version and selected protocol version. Stable protocol v1
and explicitly negotiated draft behavior are separate compatibility claims.
Do not silently enable all experimental features when updating an SDK.

When the published SDK release lacks required upstream support, an immutable
official upstream revision may be pinned instead. Record why it is needed and
verify the installed source revision, not just a package version that upstream
may not yet have changed. For Python VCS installs, verify the recorded direct-URL
commit. Re-evaluate the pin when the next official release includes the support.

## Explicit exceptions

If an upstream SDK cannot meet a required behavior, document the exact gap,
reproduction or upstream issue, affected boundary, smallest proposed augmentation,
tests, maintenance owner, and removal criterion. A replacement implementation or
fork requires explicit approval from vision owners or stakeholders before it is
introduced; implementation convenience is not an exception. Prefer an upstream
fix and a narrow wrapper in the meantime.

Review every exception when updating the SDK, adopting a newly supported
capability, or preparing a component release. Migrate to the upstream mechanism
when it covers the requirement, then remove the redundant wrapper. Record what
was retired and the behavior checks that passed.

## Compatibility evidence

Exercise both ends of each connection with the upstream SDK, including callbacks,
notifications before terminal responses, overlapping requests, cancellation,
unknown methods and metadata, malformed/oversized input, closure, and reconnect.
Preserve arrival order where it affects source incorporation or final output.
Long quiet model calls are not failures solely because no notification arrives.

Use the [upstream ACP test kit](https://github.com/agentclientprotocol/acp-tck)
alongside component tests. Report pass, fail, not applicable and not exercised
separately, with the tested revision, SDK/protocol versions and capabilities.
Passing a subset is not certification. Standard ACP compatibility and optional
Constellation behavior must have distinct results. Test tools and candidate
agents receive only the credentials, files and work allowance supplied for the
test; a compatibility check does not authorize unrelated actions.
