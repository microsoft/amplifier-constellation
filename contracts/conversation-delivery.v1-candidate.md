# Conversation delivery (candidate v1)

An agent participates in the conversation people can see. Its work may span
several model calls, tools, and incoming messages. That implementation turn must
not force all of its speech into one bubble or move later speech into the past.

## The visible experience

Keep two independent things visible:

- **Activity:** a temporary, scoped indication such as “Thinking…” or “Reading
  the project notes”. It can change as the work changes and disappear when the
  agent stops. Quiet participation evaluation produces no indicator.
- **Speech:** a complete public contribution. Post it once as a new participant
  message. It stays where it was sent. Other people and agents can speak between
  this contribution and the next one.

For example: an agent says “I’ll compare the two approaches”; a teammate supplies
a constraint; the agent later says “That rules out the first option”. These are
separate messages in that order. Its activity indicator may change throughout.
The first message is never rewritten to contain the second.

Corrections and changed conclusions are new messages. Do not turn a visible
working indicator into the final answer at its old timestamp. Do not post every
token, sentence, tool invocation, or internal reasoning step as a chat message.
The model's public communication is distinct from private reasoning.

## Boundaries and compatibility

Use the official ACP SDK and negotiated protocol. Preserve message identity,
public phase, and any documented completion signal. A network frame, timed flush,
or punctuation mark is not a complete utterance. Empty receipt envelopes do not
create a message, change its phase, or finish an older draft.

ACP v1 `agent_message_chunk` is streamed content; its optional `messageId` groups
chunks. A new identity starts a new message, but a client must still handle late
or interleaved chunks without editing published text. Prefer a documented
provider/agent completion signal where available. When the connection does not
provide a reliable boundary, retain the draft until a successful prompt terminal
instead of displaying and repeatedly rewriting incomplete prose. Do not require
a bespoke extension merely to use an otherwise compatible ACP agent.

Public commentary and final answers can both become speech. Commentary does not
establish task completion. Final-answer revision selection may replace an
**unpublished** draft; it must never overwrite an earlier published message.
Do not mistake structured participation decisions, control receipts, tool
results, or private thought chunks for public speech. A cancelled or failed
stream must not publish an unfinished draft as a completed answer.

Keep the common assembly and presentation rules near the shared ACP boundary.
Adapters own their surface's formatting, posting, and status controls. Any
optional augmentation must be documented, negotiated, covered by compatibility
tests, and reviewed as upstream ACP/SDK support improves.

## Delivery and incoming messages

Persist a stable delivery identity before posting. Distinguish prepared,
attempted, delivered, and uncertain outcomes. After a lost reply, reconcile
through the surface's supported identity/history mechanism; do not blindly
repost. Preserve the original audience and thread. Record a successful post as
part of the accessible source conversation so peers can respond naturally.
Transient activity controls stay out of agent conversation history.

An incoming message is retained even when it arrives during a participation
decision. If it invalidates the decision snapshot before any agent prompt is
dispatched, coalesce the fresh evidence into another normal attention candidate.
Do not silently classify it as delivered awareness. Actual attempted or ambiguous
steering retains its existing receipt and must not be replayed under this rule.
Edits, stops, audience changes, and existing capacity limits still apply.

Publishing a message commits what the agent said at that point. A later incoming
message can change the next contribution; it does not retract the earlier one.
Recheck relevant fresh context before effectful actions and a final commitment,
without claiming atomic freshness across independent conversation systems.

## Verification

Exercise two agents and a person in an isolated conversation, with messages
arriving during attention, generation, tool use, and final delivery. Verify:

- Speech posts append in observable order, including interleaved contributions.
- Only activity controls use edits/deletion; published speech is unchanged.
- Multiple chunks produce complete utterances, not partial-message spam.
- Unknown delivery, duplicate events, reconnects, and empty receipts do not
  duplicate speech or replay effects.
- Speech is available in context; status and private reasoning are excluded.
- Plain ACP fallback remains useful when identity/phase/completion is absent.
- The accepted outcome is actually delivered, beyond merely agreeing on a plan.

Record source IDs, decision/admission stages, public message IDs, delivery times,
and outcomes in bounded private diagnostics. Do not publish raw prompts, private
provider state, or credentials in operational status.

This is the target experience. Each component release must state which boundaries
and fallback cases it actually implements; a contract change is not deployment
evidence. See [ACP prompt turns](https://agentclientprotocol.com/protocol/v1/prompt-turn)
and [ContentChunk](https://agentclientprotocol.com/protocol/v1/schema#contentchunk).
