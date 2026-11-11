# Family composition and ownership — v1

For owners of Constellation services, connectors and consumers. The promise:
independently useful parts can evolve separately while preserving the behavior
others rely on.

## Responsibilities and connections

| Part | Responsibility |
| --- | --- |
| Instance | Installed identity, configuration, authorized context, responsibilities and installation controls. |
| Selected ACP engine | Reasoning-provider calls, execution sessions, tools and engine recovery. Constellation Runtime is one choice. |
| Conversations | Its rooms, membership, messages, source access and coordination. External platforms keep their conversations. |
| Domain connector | Platform authorization, source attribution, delivery, domain actions and connector lifecycle. |
| Experience | Interaction and presentation, using the services it needs. |
| Observatory | Fleet observation and management through installations' exposed controls, without owning their state or bypassing their permissions. |
| Umbrella | Shared direction and composition promises. Detailed APIs and tests belong to the serving component. |

[family.toml](../family.toml) locates repositories; it is not a provisioning service
or access grant. An owner may lead several components or an assembly.

## Composition relationships

Instance is the agent's logical home, hosting its selected engine. It requires no
particular container technology and is not a mandatory gateway for every ACP
client. One individual can participate in several conversations; several can
share one. Participation does not merge histories, audiences, grants or task
authority. There is no universal conversation-to-engine-session mapping.

## Experience and supporting capability ownership

Companion owns its live-call experience, including camera input, shared presentation
and useful result return. Its intended home is `amplifier-companion`; extraction
from Conversations leaves rooms, membership, messages and huddles with the service.

Amplifier is one external ACP client/experience. [Amplifier Unified](https://github.com/microsoft/amplifier-unified)
owns that product's internal clients, platforms and agreements, outside this map.

The intended `amplifier-platform-realtime` home owns reusable live-media support;
products retain call behavior and consent, engines retain reasoning and sessions.
The intended `amplifier-platform-context` home owns authorized discovery, retrieval
and context selection; sources retain records/access rules, Instance its grants,
and engines history and model-context handling. Intended homes do not establish
available APIs. Serving owners remain authoritative until an explicit compatible
handoff. Persona Workshop and ACP Lab remain independently usable supporting parts.

## Information ownership

Displaying or retrieving a record does not transfer ownership.

| Record | Authority |
| --- | --- |
| Installed identity and configuration | Instance's installation settings and controls, when used. |
| Conversation, membership and messages | Source platform or conversation service, including current access rules. |
| Execution history and continuation | Selected engine; domain-effect recovery stays with the domain. |
| Assignment | Accepting service under its contract. Recording, starting and authorizing effects are distinct; no universal task database is required. |
| External state | External system. Domain receipts record attempts, observations and uncertainty, not a replacement reality. |
| Retrieved context | Source's current authorization; excerpts retain provenance/coverage and grant no access. |
| View and draft state | Experience. Selecting or drafting is not sending or authorizing work. |

## Interface responsibilities

ACP carries client/engine interactions; MCP exposes tools and context; service
APIs expose their owner's resources and operations. A connector may both bring
input to an engine and expose domain tools. Backend reasoning-provider integration
belongs to the engine; an experience's voice/media transport remains separate.

### What crosses each connection

| Connection | Input and dependable return |
| --- | --- |
| Person ↔ experience | Direction, correction, shared material or Stop → response, inspectable work and results in the intended conversation. |
| Experience / connector ↔ Instance | Attributed input, source/destination and supported controls → scoped availability, capabilities, output and operation outcomes. |
| Instance ↔ engine | ACP discovery, supported create/load, input/context, permissions and cancellation → response, tool activity and interaction terminal. History stays with the engine. |
| Runtime ↔ tools / domain services | Authorized reads/actions → source material, observed effects or explicit failure/uncertainty. A message alone is not proof of a claimed result. |
| Experience / connector ↔ conversation source | Supported message, audience, membership and delivery operations → source-bound history and delivery evidence. |
| Agents ↔ coordination conversation | Attributed peer exchanges → inspectable coordination and meaningful return to the original request. Invitation is not execution authority. |
| Observatory ↔ installations | Authorized observation and management requests → scoped status and operation outcomes through advertised interfaces; ACP-only peers remain usable without management features. |

These are responsibilities, not new fields or mandatory sequences. Use supported,
authorized operations. Exact versions, schemas and failures live with the
[interface owner](README.md#go-to-the-interface-owner-for-details). Shared outcome
meanings live in the [interaction agreement](experience.v1-candidate.md).

## Design commitments

1. **Standard agent boundary.** Ordinary ACP supports compatible conversation
   without Instance, native Runtime or optional extensions. Negotiate extras,
   explain unsupported operations and follow the [SDK policy](protocol-sdk.v1-candidate.md).
   Label custom transports and extensions honestly.
2. **One home per promise.** Serving components own APIs and tests; consumers do
   not depend on their private files/databases. Shared agreements live here;
   explanations link them. Review linked examples when a promise changes.
3. **Identity does not erase scope.** Individuals retain separate state and
   credentials; their connections retain audiences, conversations and sessions.
   Sharing requires current authorization.
4. **Explicit engine choice.** Creation selects an engine family. Updates preserve
   it; changing families requires a separate installation, not implicit history
   migration. Model/provider choices follow supported configuration and the
   [model policy](model-support.v1-candidate.md). Catalog changes cannot silently
   alter a running task's execution context.
5. **Engagement is not effect authority.** Instance owns optional shared attention
   policy; adapters supply facts. Reasoning may choose silence. Offers, personas,
   invitations and exported responsibilities grant no execution authority; the
   resource owner enforces [task admission](task-admission.v1-candidate.md).
6. **Products choose their UX.** Reusable APIs and permissions enable different
   experiences. Admin panels and previews do not prescribe future product design.

These choices trade tighter coupling for explicit compatibility checks. Revisit
boundaries when integration and coordination cost outweigh independent reuse.

## Self-contained installation baseline

Each agent and connector owns standalone setup, authorization, renewal,
connection/image updates, recovery and disconnection. Platform-specific work
belongs to the connector, not the engine. Local tools/helpers may automate it;
external permissions still apply.

No shared provisioning, enrollment, credential, identity service, other owner's
administrator credentials or fleet dashboard is required. Optional clients invoke
installation controls without becoming sole owners of state or lifecycle.
Independent owners may use different machines, versions and implementations;
one upgrade must not require an unrelated owner's upgrade.

[Updates](installation-updates.v1-candidate.md) and
[connections](service-connections.v1-candidate.md) preserve identity, sessions,
authority, consumed work and unknown effects. Inspect effects before retrying;
never overwrite newer observations with old state. Count administrator effort
alongside end-user effort.

## Execution resources remain separate

Hosting an agent and supplying assignment resources are separate responsibilities.
Deliberately shared tools, repository objects and caches do not merge identities,
credentials, histories or grants. Separate working copies are the default; shared
writable folders require explicit choice. A room grants no filesystem access.

An execution boundary may host one or several agents. Retain Internet access
while restricting host filesystem access and preserving external authorization.
Containment grants neither host administration nor another agent's credentials;
it does not mandate a particular environment product or container technology.

## Developing the family

Use [Converge](https://github.com/microsoft/amplifier-converge) for ownership,
delegation, agreement changes and combined verification. Name an accountable
outcome owner, authority and integration check in the existing work record.
Responsibility transfers only on acceptance. Local choices within authority
proceed; settle changed shared promises before dependent work, while unaffected
work continues. Carry changed direction into assignments and checks.

[Working together](../docs/working-together.md) points contributors into that
method. Evaluate current behavior against intent; keep relevant evidence without
a second umbrella release diary or maintained gap inventory.

## Checks that matter

- Converse with an ordinary ACP peer lacking optional features.
- Set up, renew, connect, recover and disconnect without fleet services; update
  one installation without changing another's state or permissions.
- Trace composition to owned interfaces, with explicit unsupported/access failures.
- Make independent changes, then a shared-promise change: local work proceeds,
  affected owners agree and the combined result is checked, including when a
  human is unavailable.
- Change direction: affected work receives it, useful progress survives and
  obsolete plans stop.

No universal state format, automatic trust federation or shared execution service
is implied.
