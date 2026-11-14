# Conversation

For sources, connections, experiences and participants. A Conversation provides an
identifiable context for attributed exchanges, with an audience and history.

## Minimal interface

Read these operations under [composition](composition.md#reading-the-minimal-interfaces).

| Operation | What callers can rely on |
| --- | --- |
| Describe | Source-qualified identity, inspectable audience/participants, supported operations, limits and a source link where available. |
| Read | Attributed contributions with source IDs, revisions where supported, history coverage, observation time and a cursor for further history where available. |
| Contribute | Submit content to the intended conversation, with an original submission reference and any explicit attention targets; return a publication receipt and source message reference when known. |
| Inspect submission | Reconcile that submission after a lost reply, exposing the established outcome or uncertainty and the source's recovery limits. |

Read-only sources need not offer contribution. Creation, discovery, following,
membership changes, editing, search and media are advertised capabilities.
Following observes; publishing does not automatically prompt every participant.
Use [interaction](interaction.md) for attention, huddles and result return, and
[continuity](work-continuity.md) for revisions and delivery. Its
[outcome distinctions](work-continuity.md#outcomes) separate publication, agent
delivery and accepted work; references to work do not transfer its ownership.

## Reuse and addition

**Reuse:** Keep the source platform's message, thread and membership APIs. The
[Conversations API](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/api.md),
[huddle receipts](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/huddles.md)
and [context-provider interface](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/conversation-continuity.md#provider-contract-agent-instancecontext1)
are reusable implementations. Connections bridge selected input to agents;
Experiences present the exchange through these operations.

**Constellation addition:** A common description of source references, contribution
and submission outcomes, coverage and supported capabilities. Map it to the source's
existing records, including weaker recovery guarantees. An external platform can
remain authoritative; no central conversation store or new ACP method is required.

**Check:** Show a project thread in its original application and a focused studio.
Reply from each, then change source access. Can participants identify the same
conversation and its contributions? Do both views honor the source's current
access rather than maintaining competing copies?

Threads, rooms, live calls, storage and presentation remain open. A conversation
is not an engine execution session; their relationship need not be one-to-one.
Several agents can participate together, and one agent can join several
conversations without merging them.
