# Conversation awareness and closing the loop — v1 candidate

This extends [input continuity](input-continuity.v1-candidate.md) and
[Instance attention](attention.v1-candidate.md). Released behavior and adoption
belong in [implementation status](../docs/STATUS.md); a contract is not proof of
deployment or collaboration quality.

## Expected experience

People and agents participate in the same room. An agent working on a response
should receive relevant newly posted peer messages, including corrections and
accepted handoffs. Seeing a message does not mean it must answer that message.
Participants should be able to disagree, change their proposal, and deliver a
useful outcome without making the person coordinate every exchange.

## Awareness is distinct from activation

- **Activation** asks whether an idle agent should begin a contribution. Ambient
  activation remains Instance-owned attention, with its existing authority,
  source freshness, deduplication and budgets.
- **Awareness** supplies fresh conversation evidence to work already admitted
  in that same audience. It preserves the original request and authorizations,
  does not claim another attention offer, and cannot create a new task.
- A posted reply can be awareness while the recipient is working and an
  activation candidate when it is idle. Receipt of one source event does not
  justify both a redundant new task and an update to existing work.
- An agent's own output, replayed events, status cards, tool progress and approval
  controls are not new participant input. Genuine conversation text is never
  discarded just because it happens to say “Thinking…” or resembles a status.

Adapters identify source facts and route them; they do not introduce another
social classifier. Awareness retains author, source ID/revision, intake order,
conversation/thread and active work identity. Preserve it in the durable source
ledger and use the existing bounded input queue. If that queue is full, retain the
source in history with an explicit capacity receipt; never silently start work
or claim that the active model saw it. Label peer text as conversational
evidence, not a new human request or permission to expand work. Never mix evidence
from another room into the active audience without an explicit authorized policy.

## Active-only ACP extension

An Instance with a steering-capable backend may advertise:

```json
{
  "_meta": {
    "io.github.bkrabach.agent-instance": {
      "inputContinuity": {
        "awareness": {"version": 1, "activeOnly": true, "promptIdRequired": true}
      }
    }
  }
}
```

The adapter uses the negotiated `_session/steering` method, preserving its normal
input ID, prompt blocks and `idleBehavior: "promptRequired"`, with:

```json
{
  "_meta": {
    "steering": {
      "inputId": "source-revision-2",
      "idleBehavior": "promptRequired",
      "intent": "awareness",
      "activePromptId": 12
    }
  }
}
```

`activePromptId` is the original `session/prompt` JSON-RPC request ID on this ACP
connection. Instance checks that it is still pending for the same loaded session
and applies current tool and conversation authorization. Missing or malformed
references and mixed attention offers are rejected. A finished or different
prompt receives definite `promptRequired` without forwarding the update.

The adapter must not turn this awareness outcome into an ordinary prompt. On
unsupported capability or a definite idle race, the peer message remains in
normal source history for future authorized context, with an honest local
receipt. Ambiguous transport outcomes remain unknown and are not blindly replayed.
This behavior is optional, not part of standard ACP or a requirement imposed on
every ACP engine. The installation's prompt guard and the adapter's active-job
binding prevent an old awareness update crossing into a later turn.

Native runtime incorporation uses the existing turn inbox and original budgets.
An acknowledgment of receipt is not proof that a model used the new information.
The engine may revise the draft or remain silent; awareness does not require a
reply to every peer. Third-party engines retain their advertised guarantees.

## Different views for people and agents

Keep useful scoped progress visible to people, following the
[activity contract](adapter-activity.v1-candidate.md). Exclude structured adapter
controls and progress from ordinary agent-facing history, search, thread context,
attention candidates and awareness. Filtering happens before applying bounded
message limits, so transient progress cannot displace the latest conversation.

Use structured metadata and owned message IDs, including supported edited and
deleted envelopes. Do not infer control messages from text alone. Preserve
source/audience checks; filtering status does not authorize reading more rooms.
The human UI and audit records retain operational history independently.

When a person explicitly replies to an error or control message, retain that
exact thread root as a labelled reference. Include its visible text, source and
reactions in the authorized thread context, without button payloads or private
control data. It does not become a new attention candidate or an approval.
Keep unrelated status traffic filtered; never lose the subject of the question.

## Answer identity and revisions

An ACP prompt can produce several assistant messages, particularly after new
input changes the answer. Emit stable `messageId` values on text chunks and
preserve them in session replay. Interleaved chunks with the same ID belong to
that message; a new message does not truncate or complete an earlier one.
Unidentified clients retain legacy append behavior. This uses ACP v1's optional
message identity; it does not require a protocol-version change.

Provider adapters preserve assistant item identity and, where available, expose
`_meta["com.microsoft.amplifier.message"].phase` as `commentary` or `final_answer`.
Never expose private reasoning as commentary. Responses to one connector turn
select the latest non-commentary answer in message creation order, while storing
separate intermediate messages and tool history. An optional silence decision
revising an earlier draft is silence, not a malformed concatenation or a reason
to run the tools again. Media stays separately bounded and is not dropped merely
because a text message ID changes. Source evidence and delivery receipts remain
separate from the selected final text.

Malformed final output records a bounded failure stage/category, job reference
and delivery state. Distinguish an answer-format failure after completed work
from a backend/transport failure with unknown outcome. Do not leak raw provider
errors, prompts or credentials, and do not replay completed actions as recovery.

## Reactions and resource handoffs

When supported, expose current reactions through the same audience-scoped
conversation access as message text. Adding/removing a reaction uses the agent's
own service identity, only within its current authorized conversation, with a
fresh access check. Reactions are social observations; they do not grant tool
approval, create accepted work, or launch an idle agent. Active-turn reaction
observations are bounded, deduplicated and bound to the existing prompt. Ignore
the agent's own reaction echo. A removal is an observation of change, not a
claim about the complete current reaction state; query it when it matters.

For transfers of resources or ownership, distinguish requested, offered,
deposited and received, with source and time. Observe acknowledgements and
cancellations during active work. Before changing a shared plan or duplicating a
handoff, inspect recent relevant public evidence when available. A delivery
receipt establishes the sender's action, not that the recipient saw or collected
it. Keep original work limits and authorization; handoffs do not renew them.


## Reporting is part of the requested outcome

For one group result, agree on a reporter early. A nomination is not acceptance;
an acceptance is not delivery. Keep the accepted person, expected result, open
objections and reporting state available in concise, source-linked notes or
existing authorized responsibility/memory records. Do not invent commitments or
automatically expand their audience.

The reporter should provide a self-contained result with the important
compromises, unresolved issues and next decision. Others should correct material
errors or add missing information rather than repeat the same recap. Reporting
the outcome of a requested discussion does not need another permission exchange,
and agreeing on a proposal does not itself authorize implementation.

This is general installed-agent guidance and an evaluation target, not a central
leader election service or a hardcoded role. No required shared coordinator,
automatic cross-agent commitment extraction, or guarantee of one perfect summary
is introduced here.

## Freshness before delivery

Deliver awareness promptly while work is active and drain already eligible input
through the negotiated continuity path before claiming completion. Track the
latest source revision seen, source IDs submitted, incorporation acknowledgments,
and the final delivery receipt separately. Do not report full current context
merely because a transport accepted input.

Concurrent participants can still finish at the same time. A model-based final
reconsideration must be explicitly bounded, preserve completed effects and the
original allowances, and avoid an endless read/retry loop. Do not simulate that
guarantee by silently discarding drafts, restarting tools, or creating another
unbounded prompt. Actual final-check support must be documented by component;
awareness alone does not establish an atomic last-look across a distributed room.

## Evaluation

Use fictional fixtures for deterministic boundaries, then separately evaluate
the social result in bounded agent conversations:

1. A peer posts a late correction while a human-authorized task runs. The task
   keeps its effects and receives the correction in order without a new task.
2. A peer burst, duplicate delivery, self-output, edited status and late idle
   race neither multiply participation nor renew execution budgets.
3. Progress remains visible to people but cannot consume bounded model history
   or wake agents; genuine text resembling progress remains conversation.
4. A pending approval survives awareness. A stale prompt ID, another session,
   revoked source or new attention offer cannot use the awareness path.
5. A group debates real tradeoffs, records who accepted reporting, and supplies
   one useful final result without a human reminder. Evaluate stale arguments,
   redundant recaps and missing conclusions separately from transport receipts.
6. Reconnect and ambiguous delivery preserve source evidence and do not replay
   uncertain work. Diagnostics contain identifiers/stages, not raw private text.

Empty text updates may carry input receipts or other control metadata. Consumers
process that metadata independently, but ignore the empty text before assigning
message order, changing a message phase, consuming answer-group capacity, inserting
legacy text separators, or firing assistant-text callbacks. An empty update must
never create an answer placeholder that changes selection of later nonempty text.
