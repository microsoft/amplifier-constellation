# Focused specifications

These details serve implementers who need an exact shared format or convention.
Start with the [six agreements](../contracts/README.md) for cross-component promises.

| Specification | Why it remains here |
| --- | --- |
| [Public speech](../crates/acp-connection/docs/public-speech.md) | Shared message assembly and optional completion metadata, beside the common collector. |
| [Maintenance and source receipts](maintenance-and-source-receipts.md) | Exact optional Instance profiles not fully described in its broader API guide. |
| [Refreshed context envelope](context-envelope.md) | Shared producer fields and bounds; Runtime owns model-view processing. |
| [Agent portraits](agent-portraits.md) | The accepted portable visual convention, independent of product layout. |

These specifications do not require universal feature adoption. Capability
negotiation, access and component compatibility still apply. Other wire formats,
settings and behavior tests stay in their serving repositories; follow the
[interface map](../contracts/README.md#technical-detail).
