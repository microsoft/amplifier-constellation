# Shared promises

The [vision](../docs/VISION.md) explains the outcome. These six agreements cover
the responsibilities that must hold across independently owned parts. Read the
one your work changes; follow its links when you need the exact interface.

| Agreement | Question it answers |
| --- | --- |
| [Composition](composition.md) | Who owns each part, and how can we build on it independently? |
| [Interaction](interaction.md) | How do we protect human attention and return truthful, useful results? |
| [Work authority](work-authority.md) | Who can assign, change, delegate or stop work, and within what limits? |
| [Work continuity](work-continuity.md) | How does work incorporate new input and survive interruption without loss or repetition? |
| [Installations](installations.md) | How do setup, connections, updates and recovery preserve the agent and its work? |
| [Engine compatibility](engine-compatibility.md) | What can a client rely on across engines, protocols and model choices? |

Each agreement names its audience, promise, checks and choices left open. Intended
behavior is not proof of deployed support. Optional capabilities require the
serving implementation's advertised contract; a documentation change does not
activate them. [Converge](https://github.com/microsoft/amplifier-converge) governs
how owners settle changes and carry them into affected work.

## Technical detail

Exact wire formats, configuration and executable checks belong with their owner:

| Boundary | Definition |
| --- | --- |
| Agent/client protocol | [ACP](https://agentclientprotocol.com/) · [Instance attach](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/acp-boundary.md) |
| Social participation and active awareness | [Instance attention](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/attention.md) · [conversation continuity](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/conversation-continuity.md) |
| Tools, context and engine input | [Runtime architecture](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/architecture.md) · [steering](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md) |
| Installation controls | [Instance API](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/api.md) · [connections](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/service-connections.md) · [updates](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/updates.md) |
| Conversation services | [API](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/api.md) · [huddles](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/huddles.md) |
| Fleet controls | [Observatory](https://github.com/microsoft/amplifier-constellation-observatory/blob/main/docs/contracts.md) |

[family.toml](../family.toml) locates connector and other component repositories.
Focused [reference material](../reference/README.md) retains shared specifications
without another authoritative home. It is not prerequisite reading for every
contributor. Older contract URLs contain pointers only, so existing links remain
useful without preserving a competing agreement.

<a id="go-to-the-interface-owner-for-details"></a>
