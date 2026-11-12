# Independently useful parts, dependable connections

For owners building services, connectors, engines and experiences. The promise:
each part can evolve independently while keeping the behavior others rely on.

**Example:** A new experience can work with an existing agent and conversation
service without adopting their screens, copying their databases or replacing
the agent's engine.

## Responsibilities and connections

| Part | Owns |
| --- | --- |
| Instance | Installed identity, configuration, authorized context, responsibilities and controls. |
| Selected ACP engine | Reasoning-provider integration, execution sessions, tools and engine recovery. |
| Conversation source | Its rooms, membership, messages and access rules. Conversations supplies these services; external platforms keep theirs. |
| Domain connector | Platform authorization, source attribution, delivery, domain actions and connector lifecycle. |
| Experience | Interaction and presentation using supported services. |
| Observatory | Fleet observation and management through exposed installation controls. |
| Umbrella | Shared direction and composition promises, linking serving owners' APIs and tests. |

[family.toml](../family.toml) locates repositories. An owner may lead several parts
or an assembly; the map neither provisions them nor grants access.

## Commitments

1. **Compose without a central dependency.** Instance is an agent's logical home,
   with a selected engine inside. Compatible ACP clients can also use engines
   directly. No particular container or fleet service is mandatory. Follow the
   [installation agreement](installations.md) for independent setup and operation.
2. **Keep identity and scope distinct.** One agent can join several conversations;
   several agents can share one. Participation does not merge audiences, histories,
   credentials or authority. Conversation and execution-session mappings need not
   be one-to-one. Current source authorization governs retrieval and sharing.
3. **Respect record ownership.** Sources own original conversations; engines own
   execution history; accepting services own assignments. Domains own effect
   enforcement and recovery. External systems remain authoritative about actual
   external state; receipts are evidence. Experiences own drafts and view state.
   Displaying or retrieving a record transfers none of these responsibilities.
4. **Use the agreed connection.** Consumers use declared interfaces, not another
   component's private files or database. Return explicit unsupported, denied,
   failed or unknown outcomes rather than inventing compatibility or success.
5. **Give promises one home.** These contracts own cross-component behavior;
   serving components own detailed APIs and tests. Products choose their UX.
   Existing admin panels and previews do not prescribe future experiences.
6. **Keep execution resources separate.** Shared tools, repository objects and
   caches do not merge identities or grants. Separate working copies are the
   default; shared writable folders require explicit choice. A room grants no
   filesystem access. Execution boundaries may host one or several agents;
   restrict host filesystem access while retaining Internet access and external
   authorization. Neither host administration nor another agent's credentials
   follow from containment.

## What crosses each connection

| Connection | Exchange |
| --- | --- |
| Person ↔ experience | Direction, correction, shared material or Stop; responses, inspectable work and results. |
| Experience / connector ↔ Instance | Attributed input, source/destination and supported controls; scoped availability, capabilities and outcomes. |
| Instance ↔ engine | ACP discovery, supported sessions, input/context, permissions and cancellation; content, tool activity and interaction terminals. |
| Engine ↔ tools / domain services | MCP or a supported tool interface carries authorized reads/actions; returns evidence, effects or explicit uncertainty. |
| Participants ↔ conversation source | Source-owned message, audience and membership operations; attributed history, coordination and delivery evidence. |
| Observatory ↔ installations | Authorized observation/management through advertised controls; ACP-only peers remain usable without management features. |

ACP is the agent interaction boundary; MCP exposes tools/context; service APIs
expose their owner's resources. A connector may be both ACP client and domain
tool provider. An experience's voice/media transport does not take ownership of
backend reasoning. [Engine compatibility](engine-compatibility.md) defines how
these choices are tested; the [interface map](README.md#technical-detail) locates
specific operations.

## Experience and supporting capability ownership

[Companion](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/companion-vision.md)
owns live-call behavior, camera input, shared presentation and result
return; its intended home is `amplifier-companion`. Conversations retains rooms,
membership, messages and huddles. Amplifier is one external experience;
[Amplifier Unified](https://github.com/microsoft/amplifier-unified) owns its
internal products, platforms and agreements.

Reusable live-media support belongs to the intended `amplifier-platform-realtime`
home; products retain consent/call behavior and engines retain reasoning/sessions.
Authorized discovery, retrieval and context selection belong to the intended
`amplifier-platform-context` home; sources retain access rules, Instance its grants
and engines their model-context handling. Intended homes do not establish APIs;
existing serving owners remain authoritative until an explicit compatible handoff.
Persona Workshop and ACP Lab remain independently usable supporting capabilities.

## Check

Connect an ordinary ACP peer without optional features. Trace a cross-component
operation to its owner and test denied/unsupported cases. Let two owners change
independent parts, then exercise the combined journey. Verify one installation
can operate and update without unrelated installations or Observatory.

**Left open:** Product layouts, internal storage, container technology and
repository count. The deliberate tradeoff is independent reuse with explicit
compatibility checks. Revisit boundaries when coordination cost outweighs that
benefit. Use [Converge](https://github.com/microsoft/amplifier-converge) to settle
shared-promise changes; ordinary decisions within delegated authority proceed.
