# Adapter activity contract — v1 candidate

Shared guidance for every ACP experience and connector,
including chat, browser, editor, terminal, voice, and game surfaces. This defines
behavior, not a new mandatory ACP extension or shared service.

## User experience

Once an agent has committed to responding, show that it is working **in the place
where that response will appear**. While waiting for the model or composing the
next step, use **Thinking…**. While tools are active, prefer a useful, bounded,
single-line description in present tense: “Reading README.md”, “Searching project
files”, or “Running unit tests”. A tool name can add useful detail when the label
alone is ambiguous. Do not use “Preparing the response…” or generic task wording
when a more informative supported status is available.

This is an activity indicator, not access to private reasoning, proof of progress,
or a completion claim. A model wait is honestly described as Thinking… even when
its internal processing is not observable. A delivery wait can say “Sending…”.
An actual wait for a person should say “Waiting for your answer” or use the
experience's own approval/question affordance.

Do not signal work while an agent is merely considering whether to join a group
conversation. An agent choosing silence must leave no working message behind.
Direct and explicitly addressed turns may start the indicator when admitted.
Avoid a distracting flash for work completed before the indicator can be shown.

## Ownership and reuse

| Layer | Responsibility |
| --- | --- |
| Tool or provider integration | Produce the useful, public description of the operation it actually knows about. Prefer a present-tense title and a correctly classified operation kind. |
| Runtime / ACP producer | Emit standard ACP tool-call IDs, titles, kinds, lifecycle states, and partial updates. Keep the semantic label generation beside the tool implementation, reusable by every consumer. Provider-native work follows the same public activity path where it is observable. |
| Instance / transport | Preserve the session, tool-call identity, lifecycle and public presentation metadata. Do not replace it with connector-specific text or invent unseen work. Expose the same activity to its own observers. |
| Adapter / experience | Correlate the correct turn and destination, aggregate concurrent operations, select the best available presentation mechanism, and manage its lifecycle. Translate presentation, not the meaning of the work. |

Use the producer's useful human-readable title when available. Avoid prefixes
that repeat it (“Reading · Reading README.md”). Third-party ACP agents may supply
only a kind or tool name; adapters still need a conservative fallback. Share pure
normalization/aggregation code within an implementation where practical, and
validate implementations against the cases below. Do not introduce a required
Constellation runtime, network formatter, or shared management service merely to
render an arbitrary ACP agent's status. Standard ACP compatibility remains the
baseline; capabilities beyond it must be negotiated and optional.

Merge partial updates by session/turn and tool-call ID; a missing title does not
erase a prior title. Track the active set, rather than replacing it with the last
event. A completed tool leaves that set; another active tool remains visible.
When several tools run, show the most recently active useful label plus a compact
count if space permits, for example “Reading README.md · +2 tools”. Unknown IDs,
late events from an ended turn, and replayed historical events cannot start new
activity indicators. Once all tools finish and the turn continues, show Thinking….

## Choose the richest mechanism the experience supports

Prefer the first feasible, correctly scoped option; an adapter may combine a
native busy signal with a useful detail line if that improves the experience
without duplicating messages:

1. Native activity/status API with changing text: send the informative label.
2. Editable or replaceable, scoped activity element: maintain one temporary line.
3. Native boolean working/typing/busy capability: set it while responding and let
   the experience render it. Do not invent unsupported customization.
4. Static-only working affordance: use a simple “…”, spinner, or short working
   indication. Do not append a stream of tool-status messages to the conversation.
5. No suitable capability: omit the intermediate indicator; retain real lifecycle
   observations in the installation's observability interface.

A platform-wide presence flag is **not** a correctly scoped substitute for
conversation activity. A response in one channel or thread must not light up every
other room. If an experience only offers global presence, prefer a scoped local
fallback or no indicator. Platform limits can reduce detail; they cannot justify
routing activity to the wrong audience.

## Lifecycle, correctness, and failure

- Bind activity to the admitted turn, agent, conversation, thread and response
  destination. Multiple simultaneous conversations remain independent.
- Keep working state through generation, tools and delivery as appropriate. Clear
  it only when the relevant work is settled. Overlapping turns need reference
  counting or equivalent aggregation so one completion cannot clear another.
- Clear or replace temporary status on success, cancellation, failure and
  disconnection. On uncertainty, report an interrupted/unconfirmed outcome rather
  than leaving a permanent “working” signal or claiming success.
- Give a failure one visible presentation. When a detailed connector notice
  owns the outcome, retire its temporary indicator rather than posting another
  generic error. Mark connector-generated notices as control events: retain
  human visibility and explicitly requested context reads, but exclude them
  from automatic peer participation and active-turn awareness. Error reporting
  must not trigger an agent-to-agent response loop.
- Preserve delivery identity through retries. Reconcile status after reconnect;
  do not replay the underlying work to repair its indicator. Unsupported platform
  status APIs must fall back gracefully, without repeated authorization prompts.
- Coalesce duplicate labels, throttle platform writes, obey retry-after limits,
  and retain the latest desired state. Rendering must not stall agent execution.
- Use public display metadata, not raw arguments, command lines, output, tokens,
  private reasoning, or another conversation's contents. Normalize controls and
  whitespace, bound length, escape platform markup, and suppress accidental
  mentions. “Thinking…” never needs a reasoning excerpt.
- After completion, an experience with an activity-history affordance may collapse
  the details and use past tense (“Read README.md”, “Ran unit tests”). Do not
  create a chat history dump where the platform has no suitable affordance. Keep
  outcome/error evidence distinct from an activity label.

## Shared acceptance cases

Every implementing adapter should exercise the applicable cases with its own
platform fixture. Unsupported features must have a documented fallback.

| Case | Expected behavior |
| --- | --- |
| Group message, agent chooses silence | No busy signal or status message. |
| Admitted direct response, no tool activity yet | Thinking… or the platform's native busy indication. |
| Tool title followed by status-only partial update | Retain the title; reflect current activity. |
| Two tools active, one completes | Show the remaining tool, with a correct count. |
| All tools complete, model continues | Thinking…, not a claim the task is finished. |
| Same agent responds in two different rooms/threads | Each gets only its own activity and cleanup. |
| Native boolean-only surface | True during that response, false when settled; no invented labels. |
| Static-only surface | One simple indication, no tool-update message flood. |
| Failure, cancellation, reconnect or lost delivery receipt | Honest outcome and reconciled cleanup; no duplicate execution. |
| Secret-bearing input, markup, mention text or private reasoning | None is copied into the public activity label. |
| Existing installation retains an old placeholder/default | New and pending generic statuses use Thinking… after upgrade. |
| Completed expandable history | Past-tense observed operations, separate from verified outcomes. |

## Adoption

This contract belongs to the [experience baseline](experience.v1-candidate.md).
Slack's working indicators are an implementation example, not the definition of
this boundary. Minecraft, Teams and other adapters apply the same semantics to
their own supported UI. Declaring this guidance does not claim that every existing
adapter already passes every case; implementation and verification evidence must
be recorded in the owning component.

Activity indicators are control traffic. Their creation, edits and removal must
not become participant messages, new work, or context changes that invalidate
ongoing work. Where deletion events omit metadata, retain a bounded identifier
set for the indicator lifecycle. Ordinary participant messages remain unaffected.

The [input continuity contract](input-continuity.v1-candidate.md) keeps activity
attached to the logical turn when provider steering creates successor responses
or queued input requires another model call. An internal response finishing is
not the end of that activity. A durable input receipt may support a quiet
“Update received” affordance; it must not be presented as “applied.” Keep an
unchanged approval visible across ordinary follow-ups, and do not turn status
updates into source revisions that invalidate it. Release evidence for this
integration belongs to each implementing component.

Keep structured progress visible to people while excluding it from agent-facing
history, search, thread context and peer-awareness input. Apply this projection
before bounded message limits so status updates do not crowd out conversation.
Use metadata and owned message IDs, never text matching that would discard a
participant saying “Thinking…” as ordinary conversation. See the
[collaboration contract](collaboration.v1-candidate.md).

For long work and typed compaction, follow the
[harness lifecycle](harness-lifecycle.v1-candidate.md). The temporary working line
is activity, not the agent's speech. Complete public commentary is an ordinary,
permanent participant message, following the
[conversation delivery contract](conversation-delivery.v1-candidate.md). Do not
replace it with later commentary, a final answer, or a tool label. Private
reasoning and raw tool arguments remain excluded. A commentary-only turn is not
proof of a completed assignment. Use typed compaction status where negotiated
and retain unknown state when progress is unavailable.
