# Engine compatibility and explicit choice

For engine owners, protocol integrations and configuration clients. The promise:
people can choose compatible engines and models without hidden changes to their
accounts, histories or work. A declared capability has evidence at the actual
protocol and access boundary.

The SDK, negotiation and exception commitments apply to participating protocol
integrations. The selection and binding commitments apply to implementations
that expose configurable engines/models; requirements naming Native Runtime
apply to that implementation. They are active agreements for those designs, not
a requirement that every Agent Instance contain one selected engine. Other
architectures keep their [outward boundary](agent-instance.md) and the protocols
they advertise without adopting Native's catalog or internal transport.

**Example:** A catalog gains a faster model. Existing work keeps its configured
model and route. An explicit selection is validated; unsupported history or
capabilities produce a useful explanation instead of a silent substitute.

## Commitments

1. **Use the protocol owner's SDK.** Production types, framing, IDs, correlation,
   dispatch, cancellation and lifecycle use the official SDK—not merely a declared
   dependency. ACP integrations use the official [Rust](https://github.com/agentclientprotocol/rust-sdk),
   [TypeScript](https://github.com/agentclientprotocol/typescript-sdk) or
   [Python](https://github.com/agentclientprotocol/python-sdk) SDK. Apply the same
   principle to other protocols as introduced. After equivalent behavior is
   verified, remove superseded handwritten implementations. Application policy,
   identity, authorization and work accounting remain with their owners.

2. **Negotiate optional behavior honestly.** Pin SDK and protocol versions; stable
   ACP v1 and negotiated drafts are distinct claims. An SDK upgrade does not
   enable every experiment. Prefer transport adapters, middleware and extension
   points over replacement machinery. A custom authenticated transport retains
   its own identifier; ACP-shaped messages do not make it standard. Transparent
   proxies preserve foreign request IDs and unknown metadata. Additional behavior
   remains optional and negotiated.

3. **Keep exceptions narrow and removable.** An immutable official upstream SDK
   revision may fill a release gap: record why and verify installed source,
   including Python's recorded direct-URL commit. For a missing behavior, retain
   its reproduction/issue, affected boundary, smallest augmentation, tests,
   maintenance owner and removal criterion. A replacement or fork requires
   explicit vision-owner/stakeholder approval; convenience is insufficient.
   Revisit exceptions at SDK upgrades, capability adoption and releases. Prefer
   upstream fixes; remove wrappers once supported and retain the verification.

4. **Keep model policy with the engine.** In the supplied selected-engine host,
   choose the family at installation creation; changing families uses a separate
   installation rather than implicit migration of private history or active work.
   This host-specific rule does not prescribe how other Agent Instance designs
   replace their internals while preserving outward promises. Native Runtime
   maintains the latest verified useful classes for capability, speed and cost,
   with separate media and decision classes. Catalog appearance alone is not qualification; every
   historical generation need not remain executable. Runtime owns concrete IDs,
   dated capability records and provider adaptations. Configuration clients share
   those choices and validation; adapters neither duplicate catalogs nor infer
   capabilities from names. Independent engines keep their own model policies;
   no shared selection service or particular engine is required.

5. **Persist the resolved binding.** Setup or explicit update resolves a class
   to model ID, provider, endpoint/deployment and account route. Prefer versioned
   IDs. Record moving aliases and returned identities without promising stronger
   reproducibility. Publishing a catalog does not change installed configuration.
   Never change model or route during an active logical turn of that execution
   context. Independently bound workers may use different models concurrently.
   A changed binding creates an explicit execution context while preserving
   individual identity and history. Transfer public history deliberately; never replay incompatible
   opaque reasoning, signatures or private continuation state.

6. **Fail clearly rather than substitute.** Do not silently downgrade, switch
   providers or credentials, or move between subscription and API billing.
   Azure regions/deployments and gateways need their own tested route records.
   User-operated model servers remain configurable without every model becoming
   a tested class. Advertised support identifies model and access route, including
   transport, steering, tools, reasoning controls, structured output, media,
   progress and continuation; unsupported combinations are explicit.

7. **Retain the deliberate native transport choice.** Native OpenAI generation
   targets supported GPT-6-and-later models through Responses WebSockets,
   including native steering, to use ordered input during ongoing work. Retire
   older GPT generation execution and HTTP/SSE generation fallback from that path.
   HTTP still serves setup, discovery, files and other non-generation operations.
   This does not retire specialized image/decision models or promise support for
   every future model, Azure/subscription route or compatible gateway. Other
   providers use their supported transports and negotiated input fallback.
   Preserve meaningful reasoning and cost/speed choices; retire obsolete branches
   with clear migration actions while historical sessions remain inspectable.

These choices favor reusable protocol machinery and explicit compatibility over
private wire implementations or automatic substitution. Reconsider an agreed
choice with affected owners if essential interactions cannot be supported within
it; an implementation workaround must not silently redefine the agreement.

Native catalogs and transport detail live in [provider documentation](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/providers.md)
and the [steering design](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md).

**Check:** Test both ends with official SDKs: callbacks, notifications before
terminals, overlapping requests, cancellation, unknown metadata/methods,
malformed/oversized input, ordering, closure and reconnect. Quiet model calls are
not failures merely for lacking notifications. Use the unchanged [ACP test kit](https://github.com/agentclientprotocol/acp-tck)
and report pass/fail/not-applicable/not-exercised with exact revisions and
capabilities; a subset is not certification. For model promotion, check tool
ordering, reload/continuation, repeated input, cancellation, activity, negative
capabilities and original-history preservation. Separate fixtures, live-provider
and installation evidence. Tests receive only their own credentials, files and
work allowance.

**Left open:** Provider/model choice within declared capabilities, application
policy and product UX. Optional Constellation behavior is a separate compatibility
claim from standard ACP.
