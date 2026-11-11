# Find the agreement you need

Read the [family agreement](family.v1-candidate.md) for composition and ownership,
and the [interaction agreement](experience.v1-candidate.md) for behavior that
consumers can rely on. Then choose only the boundaries relevant to the next
outcome. This is a map of intended promises, not a feature or release inventory.

The family and interaction agreements define shared design commitments. Other
documents or sections retain their stated acceptance scope; candidate wording
does not silently become an agreement. Detailed component APIs remain the definition of
what an actual implementation exposes. Inspect and test that implementation
when choosing work or claiming support.

## Shared service and consumer boundaries

| If your work changes… | Shared agreement | Implementation responsibility |
| --- | --- | --- |
| How independent components compose, operate and share ownership | [Family](family.v1-candidate.md) | Affected service owners; the umbrella owns the common expectation. |
| Source, audience, destination and equivalent authorized controls | [Interaction](experience.v1-candidate.md) | Experiences and connectors, with the services they consume. |
| Protocol integration or an optional extension | [Official SDKs](protocol-sdk.v1-candidate.md) | ACP owns its standard; each component owns its integration. |
| New input during accepted work | [Input continuity](input-continuity.v1-candidate.md) | Source adapter, Instance routing and the selected engine. |
| Public speech and truthful delivery | [Conversation delivery](conversation-delivery.v1-candidate.md) | Shared assembly behavior and each destination adapter. |
| Work indicators rather than speech | [Activity](adapter-activity.v1-candidate.md) | The presenting experience or connector. |
| Human requests, limits, Stop or authorized help | [Task admission](task-admission.v1-candidate.md) | Authenticated intake and the owner of the resource being acted on. |
| Ambient participation | [Attention](attention.v1-candidate.md) | Optional Instance policy; adapters supply source facts. |
| Human priority and observed activity | [Human engagement](human-engagement.v1-candidate.md) | Instance policy, engine input and domain operation owners. |
| Peer awareness, handoffs or returning the result | [Collaboration](collaboration.v1-candidate.md) | Participants, source services and destination adapters. |
| Long execution, compaction or delivery receipts | [Harness lifecycle](harness-lifecycle.v1-candidate.md) | Selected engine, host and source-delivery owners. |
| Compact current context without losing originals | [Context snapshots](context-snapshots.v1-candidate.md) | Context producers and the consuming engine. |
| Model selection or provider compatibility | [Model support](model-support.v1-candidate.md) | The engine and its configuration clients. |
| Saved connection setup or Connect/Disconnect | [Service connections](service-connections.v1-candidate.md) | Instance registry/helper and the domain installation. |
| Updates and connected-work recovery | [Installation updates](installation-updates.v1-candidate.md) | Installation-owned providers and lifecycle participants. |
| Portable individual imagery | [Agent appearance](agent-appearance.v1-candidate.md) | Installed identity and consuming connectors; a scoped convention, not a product UX. |

## Go to the interface owner for details

- **Agent/client interaction:** [ACP](https://agentclientprotocol.com/) and
  [Instance's attach boundary](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/acp-boundary.md).
- **Tools and context:** [native tool connections](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/architecture.md#model-and-tools)
  and [authorized source continuity](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/conversation-continuity.md).
- **Installed-agent controls:** [Instance API](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/api.md),
  [saved connections](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/service-connections.md)
  and [updates](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/updates.md).
- **Conversation services:** [API](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/api.md)
  and [huddles](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/huddles.md).
- **Domain operations:** [Slack](https://github.com/microsoft/amplifier-acp-slack),
  [Teams](https://github.com/microsoft/amplifier-acp-teams) and
  [Minecraft](https://github.com/microsoft/amplifier-acp-minecraft).

[family.toml](../family.toml) locates all components. Protocol namespaces retain
their compatibility meaning even when repository names differ. A custom extension
is not made standard by its name or by appearing in this map.

For a shared-promise change, identify affected producers and consumers, preserve
or explain compatibility, and check the combined outcome. The
[working guide](../docs/working-together.md) shows how to do that without making
ordinary implementation depend on another approval.
