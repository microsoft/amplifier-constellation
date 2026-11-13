# Composing independently useful parts

For builders combining the [five primitives](README.md). The promise: use and
replace a part through its outward boundary without adopting its private design.

**Example:** A product offers an app, an agent and an investigation tool. Work
started through the tool can be inspected in the app and discussed with the agent
as the same job. Another product can use the tool alone.

## Shared composition promises

- **Roles compose freely.** A product may expose one or several primitives. They
  need not occupy separate machines, containers, processes or repositories.
  Conversations and tools may be shared. No central Constellation provisioning,
  identity or fleet service is required.
- **Identity and scope survive composition.** Preserve actor, source, conversation,
  revision, audience and return destination. Shared identity, resources or
  containment do not merge histories, credentials or authority. Retrieving context
  or displaying a control grants no additional access.
- **Records keep their owners.** Conversation sources own messages and membership;
  accepting owners own work records; resource owners determine actual effects.
  Experiences own drafts and view state. Display or retrieval transfers none of
  those duties. Entry points into the same work retain its identity and current
  evidence rather than silently create parallel jobs.
- **Interfaces carry the agreement.** Consumers use declared operations, not private
  files or databases. Specify required capabilities, versions, limits and failure
  behavior; distinguish unsupported, denied, failed and unknown. Serving owners own
  exact APIs and checks. Optional behavior requires negotiation at that boundary.

[Interaction](interaction.md), [work authority](work-authority.md) and
[continuity](work-continuity.md) each own their shared promises once. Boundary and
component contracts link to them; products choose their internal composition.

## What crosses each connection

Experiences present Conversations for human-agent communication, including private
exchanges. Connections enable participation with the conversation's source. Agents
invoke Tools through declared interfaces. Management and observation may use service
controls directly.

ACP remains the chosen interoperable agent-interaction protocol; MCP or a supported
service API exposes tools and context. [Compatibility](engine-compatibility.md)
defines SDK and negotiation obligations. This does not require an ACP engine inside
every agent or MCP for every external service. A connector can carry conversation
and provide tools. Follow the [interface owners](README.md#technical-detail).

## Experience and supporting capability ownership

[family.toml](../family.toml) locates implementations and intended homes. Companion
owns live-call behavior; Conversations owns room, membership, message and huddle
services. Amplifier's internal ecosystem belongs to
[Unified](https://github.com/microsoft/amplifier-unified). Observatory is an optional
client of exposed management controls.

Reusable media support belongs to `amplifier-platform-realtime`; experiences retain
call and consent behavior. Discovery and retrieval belong to
`amplifier-platform-context`; sources retain access rules and consumers retain
context processing. Intended homes create no API: serving owners remain authoritative
until a compatible handoff.

**Check:** Replace an implementation; trace identity, audience and work across entry
points. Test denied and unsupported requests. Independent internal changes should
still produce a working combined outcome.

**Left open:** Architecture, storage, scheduling, deployment, repositories and UX.
Use [Converge](https://github.com/microsoft/amplifier-converge) for boundary changes;
ordinary choices within an agreement remain with its owner.
