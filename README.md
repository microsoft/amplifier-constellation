# Amplifier Constellation

Persistent agents, reusable services and connections to the places people work.
Each part can develop independently while people collaborate with the same agents
across experiences.

[![Agents, experiences and an optional Observatory fleet view](docs/system-map.svg)](docs/VISION.md)

Start with the [vision](docs/VISION.md): what the parts mean, how they connect and
why they matter. Read the [contract map](contracts/README.md) for promises relevant
to your work, and [Working together](docs/working-together.md) for applying
[Converge](https://github.com/microsoft/amplifier-converge) here.

## Find the owning component

| Capability | Repository |
| --- | --- |
| Installed identity, configuration, context, responsibilities and lifecycle | [Instance](https://github.com/microsoft/amplifier-constellation-instance) |
| Native reasoning, providers, tools and durable execution | [Runtime](https://github.com/microsoft/amplifier-constellation-runtime) |
| Rooms, membership, messages and observable coordination | [Conversations](https://github.com/microsoft/amplifier-constellation-conversations) |
| External conversations and domain actions | [Slack](https://github.com/microsoft/amplifier-acp-slack) · [Teams](https://github.com/microsoft/amplifier-acp-teams) · [Minecraft](https://github.com/microsoft/amplifier-acp-minecraft) |
| Portable communication and working-style profiles | [Persona Workshop](https://github.com/microsoft/amplifier-persona-workshop) |
| Collaboration experiments and outcome checks | [ACP Lab](https://github.com/microsoft/amplifier-acp-lab) |
| Fleet observation and management | [Observatory](https://github.com/microsoft/amplifier-constellation-observatory) |

[family.toml](family.toml) indexes components, related products and intended homes,
including Companion and reusable Context/Realtime capabilities. Intended homes do
not certify available APIs. The [family agreement](contracts/composition.md)
owns their boundaries; serving repositories own setup, API definitions and tests.
The [ACP connection library](crates/acp-connection/README.md) is shared code,
not a required service.

Amplifier is one external experience, implemented by
[Amplifier Unified](https://github.com/microsoft/amplifier-unified); its internal
ecosystem stays there. [Amplifier Agent](https://github.com/microsoft/amplifier-agent)
is a distinct engine choice.

## Contribute

Name an outcome, owner, authority and combined check in the existing assignment
or PR. Local implementation choices stay with the owner; settle changed shared
promises with affected owners before dependent work relies on them. Inspect actual
behavior rather than a maintained gap list. Keep private evidence outside this
repository and shared promises in one place.

Constellation is part of the broader [Amplifier ecosystem](https://github.com/microsoft/amplifier).

## Contributing

> [!NOTE]
> This project is not currently accepting external contributions, but we're actively working toward opening this up. We value community input and look forward to collaborating in the future. For now, feel free to fork and experiment!

Most contributions require you to agree to a
Contributor License Agreement (CLA) declaring that you have the right to, and actually do, grant us
the rights to use your contribution. For details, visit [Contributor License Agreements](https://cla.opensource.microsoft.com).

When you submit a pull request, a CLA bot will automatically determine whether you need to provide
a CLA and decorate the PR appropriately (e.g., status check, comment). Simply follow the instructions
provided by the bot. You will only need to do this once across all repos using our CLA.

This project has adopted the [Microsoft Open Source Code of Conduct](https://opensource.microsoft.com/codeofconduct/).
For more information see the [Code of Conduct FAQ](https://opensource.microsoft.com/codeofconduct/faq/) or
contact [opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.

## Trademarks

This project may contain trademarks or logos for projects, products, or services. Authorized use of Microsoft
trademarks or logos is subject to and must follow
[Microsoft's Trademark & Brand Guidelines](https://www.microsoft.com/legal/intellectualproperty/trademarks/usage/general).
Use of Microsoft trademarks or logos in modified versions of this project must not cause confusion or imply Microsoft sponsorship.
Any use of third-party trademarks or logos are subject to those third-party's policies.
