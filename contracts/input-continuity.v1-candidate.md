# Input continuity while an agent works — v1 candidate

Accepted design decision: November 4, 2026. This is the intended behavior for new input
throughout a turn. It is not a statement that the released components implement
it yet. Each installation remains self-contained.

## User expectation

An eligible new message updates work in progress. It does not implicitly cancel
that work, discard completed results, invalidate an unchanged approval, or create
another copy of the task. The agent can receive many updates during successive
model and tool steps of one logical turn. Explicit stop, revoked access and
exhausted execution limits remain separate controls.

The model may revise its plan or ask about contradictory instructions. The
transport must not silently implement "last message wins," discard earlier
requirements, or claim that receiving a message means the model has used it.

## Ownership

| Layer | Owns |
| --- | --- |
| Experience adapter | Reliable source intake, source IDs/revisions, ordering at intake, truthful delivery receipts and conversation-scoped presentation. |
| Instance | Eligibility, audience/authority, routing to the correct session and active work, and capability forwarding. Ambient input still follows Instance attention. |
| Agent engine | Durable input inbox, a single session writer, model/tool sequencing, incorporation records, execution budgets and finalization. |
| Provider integration | Native steering or legal next-call injection, provider event reconciliation and preserved opaque continuation state. |

Messages from another room do not enter a running conversation merely because
the same individual participates in both. Any intentional shared session must
have an explicit audience/context policy. Historical retrieval is evidence, not
fresh input. Status cards, approval controls, replayed messages and the agent's
own output are not new participant input.

## Ordered, durable input

An accepted input record contains an installation-local ID, source event ID and
revision, author/provenance, audience binding, session ID, target turn generation,
arrival sequence and content. Assign order transactionally at intake; client wall
clocks cannot establish a reliable total order. Retain the source timestamp as
context. Different sessions have independent ordering and execution.

Deduplicate repeated delivery of the same source revision. Equal text from two
different messages is not a duplicate. A source edit after incorporation is a
new correction with its own revision, not a rewrite of provider history. An
unsent edit may replace its queued payload while retaining the revision audit.
Retractions and authority changes follow their own explicit handling.

Durability must precede acknowledgment. A bounded queue rejects excess input as
unaccepted with a retryable capacity result; it never silently drops accepted
input. The adapter keeps its durable source event until ownership is confirmed.
Do not clear a turn's source state before its admitted updates are accounted for.

The delivery ledger distinguishes:

- **Received:** saved locally and assigned to the running work.
- **Submitted:** transmitted; acceptance may still be unknown.
- **Queued remotely:** the backend owns the update but has not incorporated it.
- **Incorporated:** assigned to an identified model request or continuation.
- **Not accepted:** eligible for a deliberate retry or ordinary next prompt.
- **Outcome unknown:** requires reconciliation, not blind replay.

"Incorporated" means included in model context, not proof that the model
understood or followed it. These are independent of tool completion and task
success. Input IDs, provider submission IDs and response IDs are distinct.

## Behavior across a whole turn

| Arrival point | Required behavior |
| --- | --- |
| Before the first model request | Include the eligible queued input in order in that request. |
| During a model request | Use supported native steering; otherwise retain input for the next legal model boundary. Do not cancel the call. |
| During a tool or an approval wait | Retain new input, preserve the running operation and its real result. Keep an unchanged approval live. Explicit stop/revocation can still halt subsequent effects. |
| Between steps | Drain the queued prefix into the next request rather than launch a separate task per message. |
| During a steered continuation | Target the current response; repeat the same input lifecycle for further messages. |
| As the model finishes | Serialize input admission against finalization. Input admitted to this turn must be accounted for before its terminal result. |
| After finalization wins that race | Return an unconsumed idle outcome or durably queue a normal next prompt; never claim injection into the finished turn. |
| During a disconnect | Retain the ledger, saved results and original limits. Reconcile known outcomes before resending. |

Batching is a delivery optimization. Preserve every message boundary, author,
source identity and order in the batch. Send the first eligible update promptly;
coalesce already-waiting updates at transport or model boundaries. Avoid an
unbounded debounce timer that resets on every arrival. Use count and byte caps
and leave overflow durably queued. An early unsupported attachment cannot be
silently skipped to advance later input; surface its delivery limitation.

Maintain at most one unresolved steering batch per response chain initially.
Additional messages wait locally while that batch is being acknowledged or
committed. Drain them when the continuation is known or its required tool results
are ready. This simplifies ordering and recovery without preventing repeated
updates throughout the turn. Independent conversations remain concurrent.

## Finish work without restarting it

Retain the current call's output and completed effects. A final-looking model
answer is a draft while admitted input still needs incorporation. Continue from
that draft and the saved results. Streaming text already shown cannot be
pretended away; clearly distinguish intermediate output from the final reply.
Send a terminal ACP result only after the input/finalization boundary settles.

The inbox, finalization check and active-turn generation use a common
serialization boundary. An input admitted before the closing transition belongs
to the current turn; one arriving after it belongs to the next turn. Disconnects
or concurrent clients cannot leave an acknowledged message in neither place.

New input does not replenish model calls, tool calls, elapsed-time limits, task
deadlines or cost limits. Automatic provider continuations count too. On limit
exhaustion, report that work stopped and retain remaining input as pending;
continuation follows the existing authority and allowance policy. A busy room
must not keep one task alive forever or starve another session.

Permissions remain tied to the actual operation, source authority, installation
policy and audience. Another participant message alone does not revoke them.
Actual source retraction, explicit stop, policy revocation or a materially changed
operation can require reevaluation. A transport must not interpret an arbitrary
message, quoted text or a tool observation as a privileged stop command.

## ACP compatibility

Prefer a capability-negotiated steering interface. Current Codex and Claude ACP
implementations document the pre-standard `_session/steering` extension. Where
supported, request `idleBehavior: promptRequired` so an idle race returns input
to the client's normal prompt lifecycle rather than starting detached work.
Test the installed implementation; an upstream capability is not evidence about
an older pinned version. See the [upstream steering contract](https://github.com/agentclientprotocol/codex-acp/blob/main/docs/steering.md).

For an opaque ACP agent without steering, queue until its current prompt ends.
An adapter cannot promise injection between that engine's internal model calls.
Do not cancel-and-reprompt to imitate steering. The native runtime owns
implementation of the finer-grained model-boundary behavior. Custom metadata remains
optional and namespaced; this contract does not redefine standard ACP methods.

When a steering-capable backend definitively returns `promptRequired`, the
installation may restore that input's original attention admission for a
deferred normal prompt. The client must receive an explicit restoration receipt;
a missing or ambiguous result never authorizes replay. Restoration preserves
the original source, audience, policy, expiry and claimed budget slot. It must
not revive cancelled, revoked, superseded or expired input, or create extra
execution allowance. Provider/transport failures alone do not prove non-delivery.

A proven-unapplied restored ticket may cross a reconnect only when the installation
can verify the same authenticated principal, saved session and source-issued
audience binding using fresh authorization. It atomically rebinds and consumes
the original ticket once, keeping its original expiry and claimed budget slot.
An owner-private audience retains its actor binding. Missing proof, explicit
cancellation, changed policy/source, or an unknown delivery outcome fails closed.
This is not general ticket portability: ordinary unused offers remain scoped to
the original connection and are cancelled on disconnect.

Optional input-receipt reads and explicit recovery acknowledgments require the
same loaded-session, current conversation and tool authorization as other scoped
session controls. They do not admit new work or permit replay of an unknown
outcome. A grant revoked after session load also blocks these recovery controls.

A third-party capability has only its documented receipt guarantees. Keep the
installation's durable source ledger until delivery is reconciled; do not turn
an ordinary steering acknowledgment into a stronger claim of durable backend
incorporation. Unknown delivery must not trigger blind resubmission.

## Provider boundary

The engine selects native steering only for a verified model, route, transport
and request mode. Provider acceptance is a queued-input receipt; it does not
prove incorporation. Keep input IDs distinct from provider steering/response
IDs, track successor responses, and reconcile uncertain outcomes before retrying.
Automatic successors remain part of the original logical turn and its limits.

Where generation cannot be steered, let the in-flight call and required tools
settle, preserve their actual outputs, then insert new user input at the next
legal model boundary. The provider encoder owns tool-result adjacency, parallel
result grouping and opaque reasoning/signature preservation. Unresolved hosted
tools can require another continuation before new user text is legal. Never
hide new instructions inside a tool result or elevate retrieved text into input
authority. A final-looking response does not bypass the input/finalization check.

Provider/model eligibility follows the [model support policy](model-support.v1-candidate.md).
Concrete state machines and wire formats belong in the runtime's coordinated
[steering design](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md).
These responsibilities are accepted next behavior; runtime 0.9.0 does not yet
provide the durable inbox or native steering described here. The coordinated
[runtime 0.10.0 candidate](https://github.com/microsoft/amplifier-constellation-runtime/pull/14)
and [Instance 0.16.0 candidate](https://github.com/microsoft/amplifier-constellation-instance/pull/18)
implement these boundaries; candidate checks are not evidence of fleet rollout.

## Presentation and observability

Continue the existing scoped Thinking…/tool activity throughout the logical
turn. Do not flash completion for each internal response or add a chat message
for each queue event. An optional "Update received" indication is truthful after
durable intake. Never say "applied" at that point.

Expose queue depth/age, input state, target turn, response linkage, retries and
finalization reason through the installation's observability interface and API.
Public indicators contain no raw input, private reasoning, credentials or other
rooms' contents. This follows the [activity contract](adapter-activity.v1-candidate.md).

## Acceptance cases

Tests must check durable records and actual effect counts, not merely whether
another reply appears:

1. Many inputs during one model call, then more during successive tool/model
   steps: preserved order, authors and constraints; one logical turn.
2. A message during a local tool, hosted tool or unchanged approval: no implicit
   cancellation, duplicate action, approval disappearance or missing result.
3. Native steering acknowledgment, continuation creation and a new burst in
   either event order: correctly targeted batches with no lost input.
4. Duplicate source delivery and duplicate provider events: no second
   incorporation or second tool execution.
5. Connection loss before acknowledgment, after acknowledgment, during tools
   and after continuation creation: explicit unknown outcomes and reconciliation.
6. Input racing finalization: exactly one owning lifecycle; no detached output
   after the client believes the prompt has completed.
7. Continuous chatter and queue exhaustion: bounded resource use, no budget
   renewal, no silently dropped input and no cross-session starvation.
8. Explicit stop, source edit/retraction, access revocation and cross-room
   input: correct control handling and audience isolation.
9. Non-steering ACP endpoint: queued input is processed after completion with
   the original turn intact.

Implementation evidence belongs to the owning components. Adoption of this
candidate does not claim that these tests already pass in deployed agents.
