# Promises at the five boundaries

The [vision](../docs/VISION.md) explains the outcome. Start with
[composition](composition.md), then the boundary your work provides or consumes.
These roles can be combined in one product without becoming five separate services.

| Primitive | What another builder can rely on |
| --- | --- |
| [Agent Instance](agent-instance.md) | A continuing agent's identity, participation and accepted responsibilities. |
| [Conversation](conversation.md) | Source-owned participants, audience and attributed exchanges. |
| [Connection](connection.md) | Authorized access, participation and honest delivery into an environment. |
| [Experience](experience.md) | Human interaction with the same conversations, work and service controls. |
| [Tool](tool.md) | A callable capability with explicit inputs, outcomes and supported lifecycle. |

## Shared behavior, defined once

Read these where the behavior applies; boundary contracts link here rather than
repeat the promises.

| Agreement | Question it answers |
| --- | --- |
| [Composition](composition.md) | How do parts preserve identity, audience, ownership and compatibility? |
| [Interaction](interaction.md) | How do we protect attention and return useful, truthful contributions? |
| [Work authority](work-authority.md) | Who accepts, changes, delegates or stops work, within what limits? |
| [Work continuity](work-continuity.md) | How do correction and recovery preserve work without loss or repetition? |

## Scoped implementation agreements

These remain commitments for the implementations they name. They do not prescribe
every possible Agent Instance's internals or require every Tool to expose agent
management, execution sessions or callbacks.

- [Managed installations](installations.md): native product setup, saved
  connections, enrolled updates and verified restoration.
- [Protocol and engine compatibility](engine-compatibility.md): official SDKs,
  declared protocol support, selected-engine and model/provider commitments.
- [ACP hosting](profiles/acp-hosting.md): optional shared attention and the
  selected-engine host's input, execution and maintenance behavior.

A contract describes intended behavior, not proof of deployed support. Advertised
capabilities and combined checks establish what a particular implementation can
actually do. [Converge](https://github.com/microsoft/amplifier-converge) governs
how owners settle changes and carry them into affected work. No agreement here
adds a routine approval gate or requires everyone to read every profile.

## Technical detail

Exact wire formats, configuration and executable checks belong with their owner:

| Boundary | Definition |
| --- | --- |
| Agent/client protocol | [ACP](https://agentclientprotocol.com/) · [Instance attach](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/acp-boundary.md) |
| Participation and active awareness | [Instance attention](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/attention.md) · [conversation continuity](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/conversation-continuity.md) |
| Tools, context and engine input | [Runtime architecture](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/architecture.md) · [steering](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md) |
| Installation controls | [Instance API](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/api.md) · [connections](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/service-connections.md) · [updates](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/updates.md) |
| Conversation services | [API](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/api.md) · [huddles](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/huddles.md) |
| Fleet controls | [Observatory](https://github.com/microsoft/amplifier-constellation-observatory/blob/main/docs/api.md) |

[family.toml](../family.toml) locates the other serving repositories.
[Focused specifications](../reference/README.md) cover optional formats and
conventions. Older contract URLs remain pointers to their authoritative home.

<a id="go-to-the-interface-owner-for-details"></a>
