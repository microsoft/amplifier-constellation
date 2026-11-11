# Your agents, wherever you work

Constellation makes a team of persistent agents available across the places
people work. You can return to the same collaborator, bring specialists into a
conversation, and receive useful results without personally relaying every
exchange or following every step.

The ambition is to let a small group pursue work that would otherwise need a
much larger organization. Human attention goes toward direction, judgment,
expertise and taste. Agents carry preparation, execution, coordination and
follow-through within their accepted responsibilities and demonstrated abilities.

## A team connected to your conversations

![Several experiences contain conversations. Three individual agents each have an Instance home with a runtime inside. Connections show one individual participating in several conversations and several individuals sharing one conversation.](system-map.svg)

Read this as a picture of relationships, not a prescribed screen or deployment.
Each outlined agent home is an **instance**, with its selected **runtime** inside.
Connections let that individual participate in conversations across experiences.
The illustration shows both directions of reuse: one agent in several
conversations, and several agents in the same conversation.

The same individual does not mean one combined transcript. Each conversation
keeps its audience and access rules; each accepted assignment keeps its scope.
An agent can have several saved execution sessions. The picture does not imply
that every experience is already connected or every engine supports every feature.

## What each part means

| Part | In everyday terms | The value it brings |
| --- | --- | --- |
| **Agent / individual** | The continuing software collaborator you recognize and work with. | You can return to a familiar individual and its accepted responsibilities across different places. |
| **Instance / installation** | The agent's home. It hosts the selected runtime and keeps identity, configuration, responsibilities and connections together. | The individual remains available without starting its setup again for each conversation. This is a logical home, not a requirement for a particular container technology. |
| **Runtime / engine** | The reasoning and execution component inside the home. It works with permitted context, calls models and tools, and retains execution history. | It provides the ability to work through a request. Native Runtime or another compatible ACP engine can fill this role. |
| **Experience** | Where you interact: Companion, Slack, Teams, Minecraft, Amplifier or another connected application. | You can talk, type, show something or work alongside agents in a way suited to the setting. |
| **Conversation** | An exchange with an address, participants, an audience and a history, hosted by the experience or its supporting service. | People and agents have a shared place to ask, clarify, inspect and receive results. |
| **Domain connection** | The bridge that lets an agent participate in an experience or act in a system. | It carries attributed requests and responses and provides the supported, authorized ways to read or act there. |
| **Conversation service** | A service supplying rooms, membership, messages and observable work huddles where needed. | Experiences can reuse conversation capabilities. Existing platforms continue to own their own conversations. |

**Companion** is an experience for speaking with an agent, showing it something,
sharing a view and receiving useful results while work continues. Its product
experience is distinct from the agent it connects to and from the conversation
services it uses. The preview does not define the limits of the intended product.

**Amplifier** appears as one external ACP client and experience. Its own ecosystem
owns its internal products, clients, platforms, vision and contracts. We describe
how it connects to Constellation, rather than bringing that internal design into
this vision. Its implementation is indexed in
[Amplifier Unified](https://github.com/microsoft/amplifier-unified).

## What passes between the parts

The lines in the picture represent participation through supported connections.
They are not unrestricted access to another conversation, agent or system.

| Connection | What passes across it | What the person gets back |
| --- | --- | --- |
| **Person ↔ experience** | A request, correction, question, shared material or explicit Stop. | A useful response, a way to inspect or redirect work, and results in the intended conversation. |
| **Experience ↔ agent home** | The attributed request and its conversation, relevant permitted context and the return destination, often carried by a domain connection. | The intended individual's responses and supported work or connection status. |
| **Instance ↔ runtime** | A supported agent session, input and context, permissions and cancellation through ACP. | Response content, tool activity and an interaction outcome; the engine retains its execution history. |
| **Runtime ↔ domain tools and context** | Authorized reads and action requests through MCP or another supported tool interface. | Source material, observed effects or an explicit failure or unknown result. |
| **Experiences and agents ↔ conversation sources** | Supported message delivery, source reads and participation in rooms or huddles through owned service APIs. | An inspectable conversation and meaningful results returned to where the work was requested. |

**ACP** is the agent conversation boundary. **MCP** exposes tools and context.
**Service APIs** expose the resources and controls their service owns. They have
different jobs; their exact operations and supported capabilities belong to their
respective owners. The [family agreement's connection table](../contracts/family.v1-candidate.md#what-crosses-each-connection)
makes the shared promises precise, and the [interface map](../contracts/README.md#go-to-the-interface-owner-for-details)
leads to concrete APIs and tests.

## What working together should feel like

You ask a coordinator in Slack to prepare a customer briefing. It consults a
researcher and a specialist in an observable huddle. You can inspect or join that
discussion when useful, without its routine traffic filling your conversation.
A changed deadline reaches the relevant work without discarding useful progress.
The briefing and unresolved decisions return to the original Slack conversation.

Later, you can speak with the same coordinator through Companion. It retrieves
relevant information only where your current access permits. A familiar identity
does not expose another person's private conversation. This is an illustrative
journey to design and evaluate, not a claim of a packaged integration.

Meaningful progress, decisions, changed commitments and completion deserve the
person's attention. Routine agent coordination can remain quiet and observable.
Multiple agents can join active brainstorming or shared work without a rule that
only one may speak. Quietness alone is not success: people should be able to
understand the work, influence it and judge the result.

## Independently useful, connected by clear promises

An installation owns its setup, credentials, state, updates and recovery. It can
use a different engine, machine or release without requiring a central
Constellation service. Compatible ACP clients can also use an engine directly;
Instance is a useful home, not a mandatory gateway for all agents.

Profiles from **Persona Workshop**, independent experiments in **ACP Lab**, and
multi-installation administration through **Observatory** support the ecosystem.
They are not prerequisites for an individual to exist or respond. Reusable context
and live-media capabilities can deepen the experiences while retaining source
ownership and keeping media interaction distinct from backend agent reasoning.

People can lead substantial areas in parallel, using each other's capabilities
through agreed interfaces. The existing administration panels and previews are
ways to develop and inspect those capabilities, not the final product UX.
[Converge](https://github.com/microsoft/amplifier-converge) guides how owners and
agents turn the shared ambition into useful, independently checked work.

## How we judge the result

A useful composition carries work across independently owned parts, accepts a
correction, survives interruption and returns a result whose claims can be
checked. An acknowledgment is not completion; a lost reply leaves an honest
unknown outcome rather than a reason to repeat an uncertain effect.

Measure the human effort to set up, understand, correct and trust the system,
including administrator work. Simplify or replace mechanisms when they consume
more attention than they return. Choose the next work from this intent and actual
behavior, rather than maintaining a second inventory of gaps or old checkpoints.

This vision owns the approachable story. The [family agreement](../contracts/family.v1-candidate.md)
owns composition and information boundaries; the [interaction agreement](../contracts/experience.v1-candidate.md)
owns shared outcome meanings. Exact APIs and compatibility tests stay with their
serving components. [Working together](working-together.md) explains how to lead
and combine contributions; [the repository index](../family.toml) locates their homes.
