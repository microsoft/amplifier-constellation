# Connection

For integrations between agents and working environments. A Connection makes
supported participation, context and domain operations available through an
authorized bridge.

## Minimal interface

Read these operations under [composition](composition.md#reading-the-minimal-interfaces).

| Operation | What callers can rely on |
| --- | --- |
| Describe | Connection identity, participating ends and authorized scope, with supported profiles and references to available conversations, context, tools and controls. |
| Inspect | Observed availability and freshness, limitations and remedies; where controls are offered, the outcome of a referenced control operation. |

Use the [Conversation](conversation.md) interface for participation and
[Tool](tool.md) interfaces for callable capabilities, rather than adding a generic
dispatch operation. The Connection owns platform setup, credentials, renewal,
translation, routing and delivery bookkeeping. Sources retain conversation records;
agents retain accepted work; domain resources retain action enforcement under
[work authority](work-authority.md).

Follow [continuity](work-continuity.md) for delivery and uncertainty and
[interaction](interaction.md) for shared participation. Optional shared attention
uses the single-decision responsibility in [ACP hosting](profiles/acp-hosting.md).

A Connection may advertise a **requested return** as a tool or conversation
capability: an agent in a conversation asks for a later turn there, after a delay
or at a time, with a short stated purpose. The request grants no new authority,
audience or allowance; Stop and current access still apply when it comes due. The
Connection applies and states its own limits; this contract sets no fixed ceiling,
so tomorrow is as valid as five minutes. People in the conversation can see and
cancel a pending return, and newer human input there supersedes it. Report the
outcome honestly: scheduled, declined, superseded, cancelled or delivered. A
return exists because something specific is owed; it is not idle polling.

## Reuse and addition

**Reuse:** Native platform authorization and APIs, existing Conversation/Tool
surfaces and supported connection controls. Installable connections expose setup,
configure, connect/disconnect, renewal, revocation and recovery as supported by
[installations](installations.md). The supplied
[saved-connection interface](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/service-connections.md)
is one reusable management profile, not a required host or provisioning service.
Disconnect, credential revocation and removal are distinct operations.

**Constellation addition:** A common descriptor of the participating ends,
available interfaces and observed state. Being reachable or ready to supply
context does not establish permission for effects. Consumers need no knowledge of
the agent's engine, execution sessions or routing arrangement.

**Check:** Connect to a workspace or game server, exchange input, return a result,
lose an acknowledgment, and revoke access. Can both sides establish what crossed
and what remains unavailable or uncertain without inspecting private files?

Transport, packaging and the arrangement of routing, context and tools remain
open. A connection is a logical bridge, not a required separate process or a new
intermediary where a supported path already exists. A Minecraft connection may
provide world tools as well as conversation participation.
