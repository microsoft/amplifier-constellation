# Contract ownership

The umbrella contracts describe shared expectations. They are candidates for
review. Component-owned contracts describe concrete interfaces; they retain their
existing version names until an explicit migration changes them.

| Boundary | Owner and detailed reference |
| --- | --- |
| Family composition | [Family candidate](family.v1-candidate.md) |
| Self-contained setup, authorization, and management | [Installation baseline](family.v1-candidate.md#self-contained-installation-baseline) |
| Experience scope and programmatic interaction | [Experience candidate](experience.v1-candidate.md) |
| Working indicators and public activity across adapters | [Adapter activity candidate](adapter-activity.v1-candidate.md) |
| New messages during ongoing work and explicit stop | [Input continuity candidate](input-continuity.v1-candidate.md) |
| Current model classes, capability records and explicit selection | [Model support candidate](model-support.v1-candidate.md) |
| Active peer awareness and reporting group outcomes | [Collaboration candidate](collaboration.v1-candidate.md) |
| Ambient social admission | [Installed-agent attention candidate](attention.v1-candidate.md) |
| Self-contained update lifecycle | [Installation updates candidate](installation-updates.v1-candidate.md) |
| Standard agent/client interaction | [Agent Client Protocol](https://agentclientprotocol.com/) |
| Instance attach, custom transport, and management discovery | [Instance ACP boundary](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/acp-boundary.md) |
| Authorized conversation recall and native history | [Instance continuity contract](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/conversation-continuity.md) |
| Native optional ACP behavior | [Runtime capabilities](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/acp-optional-capabilities.md) |
| Conversation client permissions and tools | [Conversations callbacks](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/client-callbacks.md) |
| Slack participation and presentation | [Slack adapter](https://github.com/microsoft/amplifier-acp-slack) |
| Teams participation and presentation | [Teams adapter](https://github.com/microsoft/amplifier-acp-teams) |
| Portable persona packages | [Persona Workshop](https://github.com/microsoft/amplifier-persona-workshop) |
| Experiment execution and independent evaluation | [ACP Lab](https://github.com/microsoft/amplifier-acp-lab) |

Component repositories are recorded in [the registry](../family.toml).
Repository availability, an accepted contract, a component release and a verified
installation are separate facts. See [implementation status](../docs/STATUS.md)
and the owning component's release evidence.

Changing a repository name does not rename a protocol identifier or make a custom
extension part of ACP. Any namespace migration must preserve or deliberately
deprecate existing clients through the owning component's versioned interface.

The input-continuity and model-support contracts record accepted behavior;
[implementation status](../docs/STATUS.md) identifies versioned support and limits.
They require coordinated implementation and release evidence; adding these
documents does not make an older installation support them. Detailed native
execution design remains with the
[runtime](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md).
