# Conversational task admission (candidate v1)

People should be able to assign work, ask for help from teammates, change limits,
and stop work in the conversation where they are already talking. An installation
can authorize this ahead of time; each supported request need not send the person
to a dashboard or another approval screen.

This is a shared behavioral contract, not an ACP extension or an implemented
permission API. Component releases must identify the operations they support.
Existing responsibility records, attention offers and delegation envelopes do
not acquire execution authority through publication of this document.

## Policy and effective capabilities

Each installation owns its task-intake policy, defaults and ceilings. A workshop
may allow **any authenticated human participant** to request work and amendments
in its connected experience. Do not impose an owner-only DM or requester
allowlist on that configured policy. Other installations may choose a narrower
policy. Authentication and participant kind come from the connected service,
never a model-supplied name, a quoted message or a bot claiming to be human.

Expose the same effective policy through programmatic controls, the owner's UI
and bounded context available to the agent. Include:

- Whether new assignments, amendments and delegated work can be accepted now.
- Whether the installation is deliberately chat-only or temporarily at capacity.
- The applicable default duration and cumulative work limit, current usage, and
  any separate provider or tool limit that still constrains execution.
- A concrete reason for an unavailable operation and its supported remedy.

Do not infer authority from available tool names or ACP capability alone. A
temporary reply reserve does not itself authorize world or filesystem effects;
any such effect must rely on separately valid existing work authority.
Waiting for capacity cannot clear a configured chat-only mode. Installation mode
changes do not automatically resume old stopped, expired or uncertain work.

## From a request to accepted work

The reasoning agent interprets the request using its ordinary context and tools;
do not add an obligatory second model or keyword parser merely to recognize a
task. The resource-owning component validates any proposed admission against the
authenticated source and configured policy before granting execution.

Retain source identity, revision, original time, actor and audience. A supported
request becomes one durable assignment with its goal, effective limits and
source reference. Commit acceptance and its deduplication record together. Only
then may the agent say it accepted or queued the work. Queued acceptance, running
work, delivered output and verified completion remain distinct states.

A clear request within policy should receive a short acknowledgment with useful
limits, not a checklist of internal permissions. Ask for clarification only when
the missing detail materially affects the requested outcome or allowed scope.
A question about whether work is possible, general chat, or a peer's suggestion
does not by itself create an assignment.

Internal ambient/reply quotas must not silently prevent an eligible human from
requesting action or receiving a response. Route the fresh request through the
supported intake path; preserve explicit task bounds and actual provider/tool
limits. See [human engagement](human-engagement.v1-candidate.md) for priority,
observed activity and interruptible execution. This changes responsiveness, not
the provenance required for a task or amendment.

Do not ask someone to repeat an unaccepted request merely because setup lacked
the transition. Retain it as a pending request when that state is supported and
reported honestly. Admission later requires current policy and a still-valid,
unretracted source. Expired source authority, a stop, ambiguous previous effects,
or materially changed scope may require fresh confirmation. Never silently
upgrade old conversation history into a task or retry an uncertain admission.

## Changes, limits and stop

Represent each task dimension as either a finite configured limit or explicitly
no configured bound. Unknown, omitted and unavailable information must not mean
unlimited authority. A completion-bound goal still has an accepted scope, owner,
identity, Stop behavior and actual effect receipts. Installation/provider/tool
constraints remain independent, and unknown provider capacity stays unknown.

Changing a default can allow newly accepted tasks to run until completion, but
does not convert old finite, expired, stopped or uncertain assignments. Removing
an existing limit requires an authorized amendment with the same source/revision
checks and preserved consumed counts. Do not encode unconfigured bounds using
large sentinel numbers, rolling deadlines or automatically refreshed allowances.

Optional local watch/continuation helpers must advertise an input version that
can represent these facts before use. Runtime 0.14's `helper-capabilities` query
describes the separate `watch-v2` and `continuation-v2` commands; these take
`{"kind":"finite","value":...}` or `{"kind":"no_configured_bound"}` for
each dimension. Original numeric commands retain their contract. See the
[watch](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/watches.md)
and [continuation](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/continuation.md)
interfaces and executable examples. This is local helper discovery, not a new
ACP field, an Instance attention feature, or a requirement to change engines.

Even on a completion-bound task, each watch retains its finite TTL and original
identity. A new helper or relaxed task limit cannot revive expired watches,
erase delivered evidence or reset decision accounting. Advice remains separate
from execution; the resource owner rechecks current authority before effects.

A human can request a smaller or larger allowance in the same conversation when
policy permits. Bind the change to the exact assignment and expected revision,
record its source and old/new limits, and preserve consumed work, completed
effects and recovery evidence. A cumulative cap is not a fresh unused balance.
Report a rejected or constrained request honestly; do not silently grant more
than the request or the installation's ceiling.

Changing defaults affects future admissions. Extending an existing assignment
requires a separate explicit amendment; expiry never renews itself. If a lower
limit is already exhausted, stop further effectful admission and settle current
operations according to their cancellation contract. A human stop takes
precedence over queued continuations and peer requests. Do not turn a later
ordinary message into an implicit resume.

Retries use the same source/change identity to recover the existing outcome,
not a second allowance. Concurrent amendments must fail or reconcile visibly.
Unknown delivery or tool effects remain unknown until reconciled.

## Helping through other agents

When a human explicitly authorizes an agent to choose a lead or recruit helpers,
preserve that delegation scope with the original request. The human should not
have to address every helper individually. Each recipient nevertheless decides
whether to accept under its own policy, tools and available limits.

A handoff identifies the original authorization, proposed subtask, recipient,
audience, applicable limits and any permission to delegate further. In a shared
experience a recipient can verify the original service-owned source; across
experiences it needs an explicitly supported authenticated delivery mechanism.
A peer's assertion or copied JSON is not sufficient provenance. Missing support
must produce a truthful proposal or limitation, not fabricated acceptance.

Delegation cannot multiply a shared allowance. If authority grants one aggregate
budget, the executing components must support accountable allocation before
fan-out; otherwise explain that limitation. If the human authorized separate
per-agent allowances, report and enforce those as such. Handoff delivery,
recipient acceptance, incorporation and completion have separate receipts.
Private source material keeps its audience restrictions.

## Ownership and compatibility

The Instance owns shared responsibility and optional assignment capabilities.
Adapters own service-authenticated intake and enforcement for resources they
control, such as a game's action lease. A local adapter may implement task intake
with its existing tools without requiring a native Runtime or a new ACP method.
Codex, Claude and other ordinary ACP backends must retain a useful path.

The existing Instance work ledger records obligations. Its current export/import
API records delegated material and does not transfer execution authority. Keep
those meanings until a separately versioned, negotiated implementation supplies
the stronger behavior. Follow the [official SDK policy](protocol-sdk.v1-candidate.md)
for protocol additions. No shared fleet service is required.

## Verification and diagnostics

Exercise the actual admission and effect boundaries with disposable fixtures:

- A question creates no work; an eligible human request creates one assignment.
- Another eligible human can assign work under the open workshop policy; a peer
  or quoted human name cannot manufacture human authorization.
- Duplicate intake, lost acknowledgments and restart do not duplicate execution.
- Chat-only mode and temporary capacity produce different truthful guidance;
  reply admission alone grants no effects; separately valid existing task
  authority remains distinguishable from a new grant.
- New work or a mode change does not resume expired or stopped assignments.
- Amendments preserve identity and consumed counts; concurrent edits, a lower
  exhausted cap and a human stop have observable outcomes.
- Delegated acceptance retains the original scope and audience without renewing
  limits, minting authority or requiring redundant human fan-out.
- The user receives actual acceptance, progress and delivery in the originating
  conversation, following [conversation delivery](conversation-delivery.v1-candidate.md).

Keep bounded private receipts for source revision, admission/denial reason,
assignment revision, effective allowance and outcome. Do not publish private
task contents or credentials in operational diagnostics. Fixture results,
published contracts, installation configuration and naturally observed behavior
are separate evidence.
