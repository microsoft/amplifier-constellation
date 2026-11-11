# Family composition and ownership — v1

For owners of Constellation services, connectors and consumers.
The promise: independently useful parts can be developed and operated separately
while keeping the behavior that others rely on.

## Responsibilities and connections

| Responsibility | Component boundary |
| --- | --- |
| Installed individual | Instance owns identity, configuration, authorized context, responsibilities and installation controls. |
| Reasoning and execution | The selected ACP engine owns its reasoning-provider calls, session history and tool execution. Native Runtime is one implementation. |
| Conversation services | Conversations owns its rooms, membership, messages, source access and coordination services. External platforms retain ownership of their own conversations. |
| Domain connection | Each connector owns platform authorization, source attribution, delivery, domain actions and its lifecycle. |
| Composition and assistance | Experiences and optional administration clients use exposed controls. Persona preparation and evaluation remain independently usable capabilities. |
| Shared direction | This umbrella owns the family map and cross-component expectations; concrete APIs and tests belong to the serving component. |

The [registry](../family.toml) locates repositories. It is documentation, not a
required provisioning service or a grant of access. Repository count does not
determine ownership: one owner can carry several components or an assembly.

## Planned products, platforms and clients

Constellation and Amplifier Unified compose a shared ecosystem. The planned
repository homes below allocate responsibility; they do not establish available
APIs, completed extractions or running services. Intended homes are recorded under
`planned_repository` in the [registry](../family.toml); this does not assert that
each repository is absent. Creating or preparing a repository does not move
existing implementation, state or consumer dependencies.

This section records the composition boundary with Constellation. Unified's
[component-boundary contract](https://github.com/microsoft/amplifier-unified/blob/main/contracts/component-boundaries.v1.md)
retains its rules for its own composition; its
[repository map](https://github.com/microsoft/amplifier-unified/blob/main/docs/architecture/repositories.md)
locates the existing extraction boundaries. The platform names here are the
agreed target homes. Each serving owner defines its concrete interface and
compatibility tests; consumers link to that definition. During extraction the
existing owner remains authoritative until the interface moves with it. Product
visions describe their intended experience. Guides,
registry entries and diagrams navigate these homes rather than defining parallel
promises. Reusable platform code can be a library or an independently operated
service; repository separation does not require a central deployment.

### Products and client foundations

| Home | Intended responsibility |
| --- | --- |
| `amplifier-unified` | Own the Unified product experience and how it composes shared capabilities. Its planned Constellation connection uses the same owned interfaces as other consumers. |
| `amplifier-companion` | Own the Companion product: contacts, calls, voice/camera interaction, shared presentation and return of useful results. Product interaction and consent stay here; agent identity, reasoning and source conversations retain their owners. |
| `amplifier-client-web`, `amplifier-client-windows`, `amplifier-client-linux`, `amplifier-client-macos`, `amplifier-client-ios`, `amplifier-client-android` | Own reusable browser/device integration and client foundations. Products select the foundations they need while retaining their own UX and outcome responsibility. A client shell does not become the owner of agent history, work authority or every product's behavior. |

Companion's preview currently lives in Conversations. Its planned product home
allows independent development; it does not take rooms, membership, messages or
huddles out of Conversations. The existing Conversations repository remains their
home. There is no second conversation service or repository rename in this plan.

### Reusable platform capabilities

The seven capabilities being established for Unified and the Context and Realtime
homes are intended for reuse across products. These are responsibility boundaries;
exact operations and implementation choices remain with the serving owners.

| Planned home | Intended responsibility and boundary |
| --- | --- |
| `amplifier-platform-scheduling` | Own scheduling and trigger semantics. A due trigger requests work through the receiver's admission and authority rules; it does not renew a task or authorize effects. |
| `amplifier-platform-workspaces` | Own reusable workspace organization and associations among work, people and resources. Workspace membership does not silently grant access to every linked source or agent session. |
| `amplifier-platform-resources` | Own reusable file/artifact identity, content, metadata, revisions and authorized retrieval. This does not allocate compute or become a credential/provisioning authority. |
| `amplifier-platform-operations` | Own durable observations of operations, questions and tasks, with outcome correlation. This is an observation journal; executors retain control of the underlying work and effects. |
| `amplifier-platform-publishing` | Own immutable built-output snapshots and their supported preview, review, deployment and rollback lifecycle. Callers retain build and source authorization; publication does not certify an output's claims. |
| `amplifier-platform-portability` | Own supported export, import and transfer between compatible homes. A transfer does not silently migrate opaque engine history, expand grants or renew work. |
| `amplifier-platform-canvas` | Own reusable content and interaction primitives for a shared visual surface. Products own the experience; presentation or editing alone does not mean sending, approval or completion. |
| `amplifier-platform-realtime` | Own reusable live voice/media connection and interruption support. Products own call behavior and consent; selected ACP engines retain backend reasoning, tool execution and saved agent sessions. Media transport is not a replacement agent protocol. |
| `amplifier-platform-context` | Own reusable source discovery, authorized retrieval and context selection with provenance and coverage. Sources retain records and current access rules; Instance retains installation configuration and grants; engines retain execution history and model-context handling. |

Adopt these shared capabilities where they fit rather than building parallel
Constellation versions. Extract existing code through an explicit compatibility
transition with affected consumers. No new platform repository is an implicit
dependency of every installation or a universal identity, memory or credential
service. Existing [self-contained installation](#self-contained-installation-baseline)
and [information ownership](#information-ownership) commitments still apply.

## Information ownership

The components preserve different kinds of records. Displaying or retrieving a
record does not transfer its ownership or create another authoritative copy.

| Record | Owner and meaning |
| --- | --- |
| Installed identity and configuration | Instance, when used, owns the individual's installation settings and exposed controls. |
| Conversation, membership and original messages | The source platform or conversation service owns its conversations and current access rules. Conversations does not replace every platform's store. |
| Model execution history and continuation state | The selected engine owns its session format and execution recovery. Source access and domain-effect recovery remain with their respective owners. |
| Responsibility or assignment | The service that accepts it owns the record under its contract. Instance can expose responsibility records; a domain can own its task and effect limits. Recording work, starting it and authorizing effects remain distinct; no universal task database is required. |
| Actual external state | The external system owns what exists there. A connector or domain service records action attempts, observed results and uncertainty; its receipt is evidence, not a substitute for the external state. |
| Context access | The source's current authorization governs retrieval. A supplied excerpt retains provenance and coverage and does not itself grant access. |
| Display and draft state | The experience owns its supported view state; selecting or drafting does not imply sending or authorizing work. |

## Interface responsibilities

ACP carries client/engine interactions: capability discovery, supported sessions,
input, output, tool activity, permissions and cancellation. MCP exposes declared
tools and context. Service APIs expose their owner's resources and supported
operations. These are separate responsibilities, not a universal envelope or a
requirement for every client to use every interface.

A connector can be both an ACP client bringing platform input to an agent and a
tool/context service exposing permitted domain access. Its domain retains source
attribution, delivery and effect enforcement. The selected engine owns its
backend reasoning-provider integration. An experience may separately own voice
or media transport through its provider interface; this does not make those
protocols part of ACP or move backend reasoning ownership into the experience.

Exact operations, fields, supported versions and failure behavior belong to the
serving component or upstream protocol. The [interface map](README.md#go-to-the-interface-owner-for-details)
links those definitions. Inspect the actual producer and consumer before relying
on optional behavior. Shared outcome meanings live in the
[interaction agreement](experience.v1-candidate.md), including the distinction
between receipt, execution, delivery and verified completion.

## Design commitments

1. **Keep a standard agent boundary.** Ordinary ACP is sufficient for a compatible
   client and agent to conduct a conversation. Instance, the native runtime and
   optional management/context features are not mandatory for every ACP peer.
   Negotiate additional capabilities and explain unsupported operations. Use the
   [official SDK policy](protocol-sdk.v1-candidate.md); label custom transports
   and extensions rather than implying they are part of base ACP.
2. **Give each promise one home.** The serving component owns its detailed schema
   and behavior tests. Consumers use declared interfaces, not another component's
   private database or files. Shared expectations live here; guides link to them
   rather than making competing definitions. Vision explains intended value;
   contracts hold commitments; guides and diagrams illustrate and navigate them.
   Removing a guide must not remove a commitment. When a promise changes, inspect
   its linked examples in the same change rather than maintaining a separate
   synchronization inventory.
3. **Preserve identity and scope deliberately.** Different individuals retain
   separate identities, state and credentials. The same individual's connections
   retain distinct conversations, audiences and working sessions. Sharing context
   requires current authorization; identity recognition alone grants nothing.
4. **Keep engine choice explicit.** An installation selects its engine family at
   creation and may update it. Changing families means a separate installation,
   not implicit migration of private histories or active work. Model/provider
   choices within an engine follow its supported configuration and the
   [model policy](model-support.v1-candidate.md). A new catalog entry cannot
   silently change a running task's execution context.
5. **Separate reasons to engage from authority to act.** The installation owns
   optional shared attention policy; adapters supply source facts. Admitted
   reasoning can still choose silence. An offer, huddle invitation, persona or
   exported responsibility is not permission to execute. The resource owner
   validates the actual authority before effects; see
   [task admission](task-admission.v1-candidate.md).
6. **Keep products free to compose the services.** APIs and permissions define the
   reusable capabilities. Existing control panels, preview clients and test
   surfaces do not prescribe the UX of future products. A new experience can use
   the subset it needs, keeping the [interaction promises](experience.v1-candidate.md).

These choices favor independent ownership and reuse over a single tightly coupled
application. The tradeoff is explicit integration and capability checks. Evaluate
whether the boundaries actually reduce duplicated work and coordination effort;
changing a shared commitment follows the ownership process below.

## Self-contained installation baseline

Each agent and connector has a standalone path for setup, authorization,
credential renewal, connection and image updates, recovery and disconnection.
Platform-specific operations belong to the connector, not the ACP runtime.

Local platform tools and installation-owned helpers may automate that path.
External platform permissions still apply. Operation must not require a shared
Constellation provisioning, enrollment, credential or identity service, another
owner's administrator credentials, or a fleet dashboard. Optional fleet clients
invoke the installation's controls and do not become the sole owners of its
state or lifecycle.

Different owners can use different machines, versions and implementations within
the declared compatibility boundary. Updating one installation must not require
an unrelated owner's upgrade. The [update](installation-updates.v1-candidate.md)
and [saved connection](service-connections.v1-candidate.md) agreements preserve
identity, sessions, task limits, consumed work and unknown effects through their
respective operations. Recovery inspects effects before retrying; it does not
restore old state over newly arrived observations.

Measure setup and recovery effort for users and administrators together. Moving
work to an operator does not count as removing it.

## Execution resources remain separate

Hosting an agent and providing resources for its assignments are separate
responsibilities. Project tools, repository objects and build caches can be
shared deliberately while each agent keeps its identity, credentials, history
and grants. Separate working copies are the default; shared writable folders
require an explicit choice. A social room does not grant filesystem access.

An execution boundary can host one agent or several. Retain Internet access
while restricting host filesystem access; external service authorization still
applies. Containment must not silently grant host administration or another
agent's credentials. These boundaries do not choose a new environment product,
provisioning API or container technology.

## Developing the family

Apply [Converge](https://github.com/microsoft/amplifier-converge) to the current
assignment, using existing repositories, issues, PRs and agent tools:

- **Name who owns the outcome, including its integration.** Record the useful
  result, authority, limits, affected interfaces and completion evidence in the
  work's existing record. Delegation retains accountability;
  responsibility moves only when the receiving owner accepts it.
- **Decide independently within agreed boundaries.** Ordinary implementation and
  reversible experiments do not need new approval. Consequential delegated
  choices need proportionate evidence. A shared-promise change identifies its
  affected owners and is settled before dependent work relies on it; unrelated
  authorized work continues.
- **Make expertise effective.** Bring the outcome, relevant promise or example,
  evidence and unresolved question. Leave room for a colleague to reshape the
  question. Carry the resulting decision into the agreement, active work and
  checks without requiring that colleague to supervise implementation.
- **Verify the combined outcome.** Check both ends of an affected interface and
  the intended journey. A producer's claim, passing unit tests, release authority
  and a useful delivered result are separate facts. Missing evidence is not a
  pass and does not automatically require another human approval.
- **Re-evaluate from intent and reality.** Inspect current behavior when choosing
  work and when direction changes. Keep active assignments resumable, but do not
  maintain an umbrella release diary or a parallel inventory of gaps. Retain
  evidence where it supports a current decision, claim or reusable check.

[Working together](../docs/working-together.md) illustrates these commitments.
It does not assign new owners or expand anyone's installation or release rights.

## Checks that matter

- Compose ordinary chat with an ACP peer that lacks every optional extension.
- Operate two independent installations with optional fleet services absent;
  update one without changing the other's state or permissions.
- Exercise standalone setup, renewal, connection, recovery and disconnection,
  counting administrator effort as well as visible user steps.
- Trace each cross-component call to an owned interface, with explicit capability
  and authorization failures instead of an undeclared private-state dependency.
- Give two owners independent changes, then a change to a shared promise. Verify
  that local work proceeds, the affected promise is negotiated, and the integrated
  result is checked. Repeat with a human temporarily unavailable.
- Change the intended outcome. Affected work must receive the new direction;
  useful progress stays, obsolete plans do not continue by inertia.

This agreement does not define a universal agent-state format, automatic trust
federation or a required shared execution service. Any such choice needs its own
concrete purpose and agreement.
