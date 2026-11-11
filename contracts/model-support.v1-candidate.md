# Current model classes and explicit selection — v1 candidate

Accepted direction. This defines the support policy for the
Constellation native runtime and the configuration clients that use it.
Inspect the selected engine, its declared capabilities and relevant checks when
relying on support. Independent third-party ACP engines retain their own model
policies.

## Support current choices

Maintain the latest verified model in each useful class for each provider.
Classes preserve meaningful choices among capability, speed and cost; specialized
image, audio and decision models have their own classes. The goal is not to keep
every historical generation executable indefinitely. A model appearing in an
upstream catalog is a candidate for support, not evidence that our integration
works with it.

The runtime owns the dated catalog, concrete model IDs, capability records and
provider protocol adaptations. Instance, CLI and other configuration clients use
those same exposed choices and validation. Adapters do not duplicate catalogs,
infer capabilities from model names, select a substitute model, or require a
particular engine family. There is no required shared model-selection service.

## Make the resolved choice visible and stable

Resolve a class during setup or an explicit update, then persist the concrete
model ID, provider, endpoint/deployment and relevant account route. Prefer a
versioned ID when the provider offers one. If only a moving upstream alias is
available, record that limitation and the returned model identity when supplied;
do not imply a stronger reproducibility guarantee.

Publishing a new catalog does not silently change installed configuration.
Never change models or routes between calls of an active logical turn. A
provider/model binding change creates an explicit new execution context while
preserving the individual's identity and saved history. Transfer of public
history is deliberate; opaque reasoning, signatures and private continuation
state must not be replayed into an incompatible model.

Unavailable models produce an actionable configuration result. Do not silently
downgrade, switch providers, change credentials or move between subscription and
API billing routes. Azure deployments, regional availability and compatible
gateways may differ from a provider's primary API. User-operated model servers
remain configurable without every arbitrary model name becoming a tested class.

## Validate the actual capability boundary

Capability records identify the model and access route, with explicit support or
limits for transport, steering, tools/tool choice, reasoning controls, structured
output, media, progress events and continuation. Unsupported combinations fail
clearly before being advertised. Provider family or model-name prefixes alone
cannot establish these capabilities.

For the native OpenAI generation path, the baseline is supported
GPT-6-and-later models using Responses WebSockets, including native steering.
Retire older GPT generation execution and its HTTP/SSE generation fallback from
that path. HTTP remains useful for setup, model discovery, files and other
non-generation operations. This does not remove specialized image/decision
models, or assert that every future model or OpenAI-compatible endpoint supports
the same protocol. Azure, subscription authorization and gateways require their
own tested route records. Other providers keep their appropriate transports and
the [input-continuity fallback](input-continuity.v1-candidate.md).

Preserve meaningful supported reasoning-effort and cost/speed choices when
updating a class. Remove obsolete generation-specific branches when current
verified behavior replaces them, while retaining migration fixtures and saved
records that protect existing work. Retired execution support must fail with a
clear migration action; historical sessions remain inspectable.

## Promotion and migration checks

Before promoting a class replacement, record:

- The exact model/route, catalog date and resolved configuration.
- Tool execution and real result ordering, saved-session reload/continuation,
  repeated input during work, cancellation and public activity behavior.
- The native capabilities offered for that class, including explicit negative
  checks for unsupported combinations.
- Original-history preservation and an explicit new context for incompatible
  bindings, without replaying uncertain effects or resetting work allowances.
- Whether evidence came from protocol fixtures, a live provider or an actual
  installation update. One does not stand in for the others.

Concrete catalogs and protocol details belong in the runtime's
[provider documentation](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/providers.md)
and coordinated
[steering implementation design](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md).
An installed engine retains its configured binding and advertised capabilities
until explicitly updated. Evaluate the actual route before relying on a change.
