# Human engagement and continuous action (candidate v1)

People should be able to get an agent's attention and redirect its work in the
experience they are using. Internal counters intended to limit ambient chatter
must not silently make the agent ignore a fresh human request. An agent should
stay usefully engaged during actual human interaction, without manufacturing
conversation or work merely because someone is logged in nearby.

This records accepted behavior. Implementation, capability negotiation, deployment
and real interaction remain separate evidence. It does not create a new ACP
standard, a second agent identity or a shared provisioning service.

## Priority and limits

Separate three kinds of input:

1. **A fresh human message.** An authenticated experience supplies the actor and
   participant kind. Known direct conversations use ordinary ACP without an
   obligatory social judgment. In shared spaces Instance can still judge the
   actual addressee and relevance, but its ambient decision, turn and uncertainty
   quotas do not suppress a source-attributed human message. Human messages do
   not spend those ambient quotas. Unknown participant kind is not proof of a
   human; a quoted name or a peer claiming to speak for a person is insufficient.
2. **Observed activity.** Recent movement and interaction can make an agent
   useful without another chat message. The source provides bounded facts with
   original time and revision. Instance decides whether to engage. Activity is
   not human speech, task authorization, guaranteed need for a reply, or an
   exemption from ambient accounting. Idle presence, unchanged repeated
   observations and the agent's own activity do not keep it awake.
3. **Peer and background input.** Preserve useful coordination and accepted-work
   continuation while keeping ambient accounting and deduplication. An agent
   cannot acquire human priority by copying a person's request into its own
   message. Delegation follows the separately supported task-admission contract.

Adapters must not put an exhausted conversational reserve or automatic-work
throttle ahead of an eligible human request. Retain the request and process it
through the supported human path; do not silently consume it as a hold or ask
the person to wait for an arbitrary local hourly reset. Do not manufacture a
reply to every event: a real hold means that the evidence does not call for this
agent, not that a reply quota was exhausted.

Distinct human requests must not disappear just because a newer movement, peer
update or other message arrives. Preserve each eligible request until it is
handled, changed at its source, explicitly cancelled or expired. Supersession of
ambient context is not implicit cancellation of human input. Preserve duplicate
rejection, original occurrence time and existing ticket expiry; this is not
a promise to replay uncertain delivery or retain an offer indefinitely.

Stop, revoked access, actual tool permissions, provider service limits and
physical execution constraints remain effective. A human stop has precedence
over queued work. User-requested task bounds and completed effects remain in
the task record. Fresh requests and explicit amendments can authorize further
work under the installation's supported policy; activity and ordinary courtesy
do not silently renew expired tasks. If a real constraint prevents action,
explain the relevant limitation briefly in the originating conversation and
retain an accurate outcome. Do not disclose private operational details or
pretend provider failure when a local admission check was responsible.

## Activity facts and optional host support

The existing Instance attention extension can advertise:

```json
{
  "humanMessages": {"version": 1, "budgetPolicy": "independent_of_ambient"},
  "activity": {"version": 1, "contextField": "activities", "maxItems": 16}
}
```

These are additive fields inside `_meta["com.microsoft.amplifier.attention"]`,
whose version, enabled flag and method must also be checked. The human-message
policy uses the existing participant `is_agent:false` fact; no new message flag
is sent. An omitted participant kind remains unknown. No new fields are added
to standard ACP types outside their supported metadata containers.

Only when `activity.version == 1` is advertised may the existing attention
request include `context.activities`:

```json
{
  "sessionId": "saved-session",
  "occurredAt": 1790000000000,
  "context": {
    "conversation_id": "shared-space",
    "agent": {"id": "agent", "name": "Helper", "is_agent": true},
    "participants": [
      {"id": "agent", "name": "Helper", "is_agent": true},
      {"id": "person", "name": "Participant", "is_agent": false}
    ],
    "messages": [],
    "activities": [{
      "id": "movement-event-1",
      "actor": "person",
      "kind": "movement",
      "summary": "The participant approached the shared work area.",
      "revision": "source-revision-1",
      "occurred_at": 1790000000000
    }],
    "trigger_id": "movement-event-1"
  }
}
```

Supported kinds are `movement` and `interaction`; neither means a heartbeat or
idle presence. The adapter supplies observed facts, not a second social verdict.
IDs must be unique across messages and activities in the request. Each activity
has a known actor, nonempty revision (at most 128 bytes), source ID (at most 512
bytes), positive original time and nonempty summary (at most 2 KiB). The trigger's
time must equal the outer `occurredAt`; retries cannot refresh it. Future times
outside the existing clock allowance are invalid. There are at most 16
activities, 40 combined messages/activities, 64 participants and 32 KiB of total
evidence. A trigger cannot be this agent's own activity.

Coalesce source samples into meaningful observations with stable source identity;
do not emit a decision per render frame or reset a debounce indefinitely. A
coalesced observation keeps the original occurrence of its actual trigger and
reports its source revision honestly. Existing source-delivery facts can record
delivery/incorporation of an activity; that never proves an action completed or
a person received a reply. Do not expose other audiences through the summary.

Older hosts reject unknown context fields. When support is absent, keep activity
in authorized context/tool observations or the adapter's explicitly configured
ordinary-ACP fallback. Do not fabricate a chat message, silently install a second
social model, or claim low-cost host filtering where it is unavailable. An
adapter migration and real human-activity validation are separate from the host
implementation.

## Planning while actions execute

Reduce the cycle of one small action followed by a long planning pause through
interruptible domain action sequences and planning ahead. Prefer the experience's
existing tool/operation mechanism over a new agent transport. A tool may return
a durable operation ID and `accepted`/`running` result so the model can consider
the next action while execution continues. That result must not report completed
physical effects. Status, observed results and cancellation belong to the same
operation identity and original task authority.

Every sequence step rechecks current authority and relevant world/resource
preconditions before effects. A stop, retraction, changed condition or failed
dependency prevents unstarted steps; already started operations settle according
to their cancellation contract. Record actual effects, including partial and
unknown outcomes. Do not replay a possibly accepted sequence after a lost reply.
Count each real effect against the original allowance; batching is not a way to
obtain extra authority. Mutually exclusive operations remain serialized by the
component owning the resource.

There remains one controlling prompt lifecycle per saved engine session. Use
negotiated steering for new human guidance; do not start a concurrent prompt in
the same session to simulate parallel planning. With an opaque backend that
lacks steering, the adapter can queue input until the current prompt finishes;
it cannot inject between that engine's internal model calls. Tool result
adjacency and execution scheduling belong to the engine. An async domain
operation can permit overlap, but does not prove every runtime will plan ahead.
Independent agents retain independent sessions, credentials and budgets.

## Progress, silence and diagnostics

Successful operations should yield durable observed progress. A missing optional
model-written summary must not be reported as ordinary capacity waiting or
silently erase successful work. If further execution really needs review, retain
the effects and state the actual missing prerequisite. An agent may continue
valid useful work without repeated human nudges. Completed, stalled or impossible
work does not justify repeating empty planner turns, and fresh engagement can
still warrant conversation after work ends.

Keep status separate from speech: use the experience's working indicator during
active response, post complete useful utterances as separate immutable messages,
and keep routine checkpoint/receipt bookkeeping private. Silence means sending
no conversational content; do not post a message announcing nothing to add.

Private observability must distinguish source arrival, actor kind, activity or
message, decision attempted/held, admission class, queued/forwarded/incorporated
input, actual operation progress and delivered speech. Record reasons and bounded
identifiers rather than transcripts or credentials. Measure response latency and
useful progress; separate hard constraints, ambient quota holds, semantic holds,
provider errors and actual quiet periods. Do not equate a forwarded turn, an
incorporation receipt, or an active service with successful human interaction.

## Acceptance and ownership

- Exhaust ambient decision and turn limits, then verify a fresh human message
  can still be evaluated and admitted without replenishing ambient capacity.
- Verify another addressee, explicit refusal, stop, revoked access, duplicates,
  expired offers and unknown participant kind retain their intended outcomes.
- Compare recent meaningful human activity, idle presence, peer movement and an
  agent's own output. Preserve original timestamps and source/audience isolation.
- Interrupt a running action sequence and send multiple updates; preserve order,
  real completed effects and a single prompt lifecycle, with no duplicate work.
- Exercise no-steering backends and async operation errors honestly; do not claim
  capabilities based only on a runtime family name.
- Show a natural human exchange and useful task progress after deployment.
  Fixture results do not substitute for that observation.

Instance owns shared engagement policy and negotiated activity admission;
Runtime owns provider input continuity and model/tool scheduling. Adapters own
source attribution, domain operation execution, physical cancellation, task
authority and delivery. This contract builds on [attention](attention.v1-candidate.md),
[task admission](task-admission.v1-candidate.md), [input continuity](input-continuity.v1-candidate.md)
and [conversation delivery](conversation-delivery.v1-candidate.md).

Current protocol guidance: [ACP prompt lifecycle](https://agentclientprotocol.com/protocol/v1/prompt-turn),
[tool status](https://agentclientprotocol.com/protocol/v1/tool-calls),
[extensions](https://agentclientprotocol.com/protocol/v1/extensibility) and the
[upstream steering extension](https://github.com/agentclientprotocol/codex-acp/blob/main/docs/steering.md).
Use official SDKs for these boundaries. Do not imply that this host profile is
standard ACP or that a pending domain operation has completed.
