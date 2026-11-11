# Family composition and ownership — v1 candidate

Review draft. For owners of Constellation services, connectors and consumers.
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
   rather than making competing definitions.
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
