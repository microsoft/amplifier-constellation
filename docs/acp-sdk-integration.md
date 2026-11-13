# ACP integration boundaries

<a id="keep-wrappers-narrow"></a>

[Engine compatibility](../contracts/engine-compatibility.md) owns SDK use,
capability negotiation, exceptions and verification. This page locates the
implementations and contribution guidance.

<a id="reuse-the-protocol-integration"></a>

| Work area | Start here |
| --- | --- |
| Shared Rust connection, relay and application queues | [ACP connection library](../crates/acp-connection/README.md) |
| Shared speech assembly | [Speech interface and collector](../crates/acp-connection/docs/public-speech.md) |
| Constellation Instance host attach and authenticated transport | [Host ACP boundary](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/acp-boundary.md) |
| A component's protocol integration | [Component index](../family.toml), then its local interfaces and checks |

Read the serving component's dependency manifest and lockfile for its actual
SDK pin. The connection library is reusable compiled code, not a required service.

<a id="evaluate-the-connection-you-are-changing"></a>

Use [Working together](working-together.md) to scope the outcome with affected
owners and identify the combined check. Exact interfaces and executable checks
remain with their serving components.
