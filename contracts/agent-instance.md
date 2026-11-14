# Agent Instance

For agents and their callers. An Agent Instance is an independently addressable,
continuing participant. Its outward behavior does not require callers to know
how it works inside.

## Minimal interface

Read these operations under [composition](composition.md#reading-the-minimal-interfaces).

| Operation | What callers can rely on |
| --- | --- |
| Describe | Stable agent identity, supported interactions and relevant availability. A name or avatar is not the identity; changing a process or machine need not create another agent. |
| Interact | Attributed input and responses associated with the source conversation, original input/revision and any relevant accepted work. |
| Inspect input | Recover the disposition of an original submission and its work reference, if any, after a lost response. Disclose reconciliation and retention limits. |
| Read / change accepted work | Where work is accepted, inspect its responsibility and use supported amendment or Stop operations against that same work. |

An authorized work view identifies the work and responsible agent, scope and
limits, revision, origin and result destination, current status, outcome/evidence
references and available controls. [Work authority](work-authority.md) and
[continuity](work-continuity.md) own acceptance, amendment and recovery semantics.
Internal delegation does not make callers coordinate hidden workers. Management
is separately advertised; ordinary interaction requires no administration access.

## Reuse and addition

**Reuse:** Standard [ACP v1](https://agentclientprotocol.com/protocol/v1/initialization)
provides negotiation, authentication, sessions, prompts, updates and current-turn
cancellation, with capabilities for additional interactions and session loading.
An ACP session is not a Conversation or accepted-work identity; `session/cancel`
alone does not revoke continuing work. Existing
[Instance participation](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/conversation-continuity.md)
and [work controls](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/responsibilities.md)
provide implementation inputs, not required internals.

**Constellation addition:** Bind stable agent, source, actor, conversation, input
and work references to those interactions, and expose input reconciliation and
accepted-work controls. Reuse ACP metadata and
[negotiated extensions](https://agentclientprotocol.com/protocol/v1/extensibility)
or existing owner controls; this contract does not assign new ACP method names.
The outward agreement survives replacement of the implementation.

**Check:** Change a coordinator from one engine to an inner team. Continue its
conversation and existing work. Can callers recognize the same agent and obtain
its outcome without learning the new arrangement? Lose an acceptance response and
recover its disposition without another assignment. Exercise amendment and Stop
against the same work, including while reconciliation is pending.

Engines, models, workers, memory, scheduling and deployment remain open. These
examples are not a fixed menu. An internal participant becomes another public
Agent Instance only when exposed with that role and its promises.
