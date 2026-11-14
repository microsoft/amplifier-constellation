# Tool

For capability providers and callers. A Tool offers a defined capability without
requiring an agent to absorb its machinery or treat it as a social participant.

## Minimal interface

Read these operations under [composition](composition.md#reading-the-minimal-interfaces).

| Operation | What callers can rely on |
| --- | --- |
| Describe | Capabilities, accepted inputs, results, operating limits and supported lifecycle. |
| Invoke | A result, explicit failure or reference to continuing work, associated with the request. |
| Observe / retrieve | For work outliving its initial response, inspect the same operation and retrieve its outcome under stated retention and recovery limits. |
| Supply input / continue | Where supported, correlate an answer or direction with its operation/question. State whether continuation keeps that operation or creates a linked follow-up. |
| Cancel | Where supported, request cancellation and expose its disposition under the operation's cancellation contract. |

Describe/invoke are the baseline; longer-running capabilities add retrieval and
advertise progress, input and cancellation support. The provider owns its accepted
operation; the calling Agent Instance owns its place in the larger assignment and
the meaningful return to people. Apply [work authority](work-authority.md) and
[continuity](work-continuity.md), including its [outcome distinctions](work-continuity.md#outcomes).
An investigation can finish successfully while finding defects in its subject.

## Reuse and addition

**Reuse:** [MCP tools](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)
provide discovery, schemas, invocation and results, including resource references.
For compatible clients, the optional
[MCP Tasks extension](https://tasks.extensions.modelcontextprotocol.io/specification/2026-07-28/tasks)
provides handles, `tasks/get`, `tasks/update`, `tasks/cancel` and task-status
notifications. Active-request progress is not a durable operation record.
[MCP elicitation](https://modelcontextprotocol.io/specification/2026-07-28/client/elicitation)
retains its human-input semantics; agent-answerable domain questions use the tool's
typed input or continuation capability.

[Smart Tools](https://github.com/microsoft/amplifier-smart-tools) defines a
library-first format with a thin CLI, discoverable guidance and optional adapters.
Its general long-running and
clarification conventions remain [open](https://github.com/microsoft/amplifier-smart-tools/blob/main/ROADMAP.md).
Reuse a provider's implemented lifecycle without claiming a universal Smart Tools
job protocol or requiring MCP for a callable library or existing service.

**Constellation addition:** Correlate caller, original request, provider operation,
questions and outcomes across those surfaces. Use the provider's records and declare
continuation/recovery limits; no parallel job ledger or callback service is needed.
Questions return to the caller, who answers within its assignment or seeks the
needed human input under [interaction](interaction.md).

**Check:** Two agents commission investigations. Interrupt one connection, then
retrieve outcomes and answer a clarification. Do operations remain distinguishable
and avoid blind resubmission or questions going to the wrong caller?

Intelligence, duration, hosting, progress delivery and internal agents remain open. A simple tool
needs no job service, persona or human-conversation membership. Its product may
also expose an experience or Agent Instance using the same underlying work.
