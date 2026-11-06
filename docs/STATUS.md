# Implementation status

November 6, 2026. This is a dated development checkpoint, separate from the vision
and draft umbrella contracts. Downloadable releases are listed by each component. Source-only components are
identified below. Release availability is not an assertion that every installation
has been updated.

## Current release: long work, context and visible progress

[Runtime 0.11.1](https://github.com/microsoft/amplifier-constellation-runtime/releases/tag/v0.11.1)
separates provider inactivity from whole-turn and tool limits, retains original
history beside its compacted working context, and adds native OpenAI/Anthropic
compaction, cache-aware stable prefixes and content-free execution measurements.
OpenAI uses explicit compaction at settled response boundaries: the live API
rejects steering together with automatic compaction. Updates arriving during
maintenance remain ordered, while already accepted steering and completed effects
stay on their original chain. Version 0.11.1 supersedes 0.11.0 for this combination.

[Instance 0.17.1](https://github.com/microsoft/amplifier-constellation-instance/releases/tag/v0.17.1)
pins that runtime for new native installations. Its optional source-delivery
receipts let attention distinguish an update already incorporated during work
from one whose delivery remains unknown. Incorporation is not completion or an
automatic decision to suppress another reply. Guarded backend compaction requires
an advertised command, an idle session and matching typed terminal success.
The dashboard and API expose native context controls and measurements. Existing
installations also need their enrolled runtime component updated.

[Slack 0.11.0](https://github.com/microsoft/amplifier-acp-slack/releases/tag/v0.11.0),
[Conversations 0.8.3](https://github.com/microsoft/amplifier-constellation-conversations/releases/tag/v0.8.3)
and [Observatory 0.7.3](https://github.com/microsoft/amplifier-constellation-observatory/releases/tag/v0.7.3)
surface public commentary and work/compaction status without treating it as a
final answer or displaying private reasoning. Slack negotiates source-delivery
facts while ordinary ACP agents retain their existing path. See the
[harness lifecycle contract](../contracts/harness-lifecycle.v1-candidate.md) and
the Lab's [matched experiment guidance](https://github.com/microsoft/amplifier-acp-lab/blob/main/docs/harness-experiments.md).

All five native release targets passed their required CI checks. Controlled live
OpenAI and Anthropic compaction checks passed; a live OpenAI compaction/steering
check incorporated two updates while preserving one requested file write. An
explicitly labeled synthetic Slack check verified two updates during one tool
execution, the revised final answer, scoped status cleanup and parent/thread
context. These establish the tested boundaries, not universal model support,
human participation quality or long-run cost savings. Installation verification
covered managed Linux and macOS hosts; private identities and deployment records
remain outside these repositories.

## Coordinated release: input continuity and model support

The [input continuity contract](../contracts/input-continuity.v1-candidate.md)
and [model support policy](../contracts/model-support.v1-candidate.md) define the
coordinated Runtime 0.10.0, Instance 0.16.0 and Slack 0.9.0 change. The runtime's
[overview](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/runtime-overview.md)
and [steering design](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md)
describe the implementation and its recovery boundaries.

[Runtime 0.10.0](https://github.com/microsoft/amplifier-constellation-runtime/releases/tag/v0.10.0)
adds a durable ordered input inbox within one active prompt per session, native
GPT-6+ Responses WebSocket steering, and legal next-call injection for other
native providers. Input receipts distinguish incorporation from uncertain
outcomes; completed effects and actual tool results stay part of the same work.
[Instance 0.16.0](https://github.com/microsoft/amplifier-constellation-instance/releases/tag/v0.16.0)
applies current conversation and tool authorization to steering and receipt
recovery. A definite unapplied `promptRequired` result can restore the original
attention ticket once, retaining its expiry and budget slot across a verified
same-principal/same-session reconnect. Unknown delivery is not replayed.
[Slack 0.9.0](https://github.com/microsoft/amplifier-acp-slack/releases/tag/v0.9.0)
provides ordered intake, preserves ordinary follow-ups and unchanged approvals,
and negotiates steering where the engine supports it. Opaque non-steering
engines receive a queued next prompt.

The runtime owns current model classes and defaults. Instance uses its catalog
for suggestions, explicit provider defaults and compatible account choices.
Retired settings remain inspectable for repair; configuration/execution validation
stays strict. Loading settings does not silently change models, saved provider
bindings, authentication routes or existing history.

Protocol and process fixtures cover repeated input, provider event ordering,
unknown outcomes, lost acknowledgments, prompt finalization and ticket reconnect
races. A disposable three-component Slack/Instance/Runtime check also passed
using controlled model/Slack fixtures. These checks are separate from live
provider, human experience and installation rollout evidence; publication does
not imply every installation has been updated.

The browser follow-up adds durable intake in
[Conversations 0.8.0](https://github.com/microsoft/amplifier-constellation-conversations/releases/tag/v0.8.0)
and [Observatory 0.7.0](https://github.com/microsoft/amplifier-constellation-observatory/releases/tag/v0.7.0).
Sending stays available while work runs. Each recipient negotiates steering or
queues the next prompt; ordinary input never cancels work. Delivery receipts,
explicit Stop, reload recovery and requester-bound approvals remain distinct.
Uncertain input is not replayed, and continuing after uncertainty is explicit.

[Instance 0.16.1](https://github.com/microsoft/amplifier-constellation-instance/releases/tag/v0.16.1)
and [Slack 0.9.1](https://github.com/microsoft/amplifier-acp-slack/releases/tag/v0.9.1)
add active-prompt-only peer awareness. A peer correction can reach existing work
without a new attention offer or task. Idle races retain source history instead
of starting another prompt. Bounded awareness queues and structured progress
filtering protect useful context. Generic reporting guidance distinguishes
nomination, acceptance and delivery of a group result. See the
[collaboration contract](../contracts/collaboration.v1-candidate.md).

Process and real-browser fixtures cover ordered bursts, mixed agent capabilities,
lost acknowledgments, interrupted delivery, retained approvals and recovery
without duplicate effects. These checks do not establish an atomic final read
across a distributed room or guarantee one ideal group summary. Both need
separate experience evaluation; late peer evidence remains auditable.

## Collaboration follow-up: coherent answers and scoped context

Runtime 0.10.1 preserves assistant message identity through provider streaming,
steering successors and session replay. Slack 0.10.0, Conversations 0.8.1 and
Observatory 0.7.1 assemble interleaved chunks separately and select the latest
non-commentary answer for delivery. A revised silence decision no longer becomes
a malformed concatenation or an excuse to repeat completed tools. Instance 0.16.2
uses the same rule for background reports while charging all emitted output
against the original budget, and pins Runtime 0.10.1 for native installations.

Slack retains a specifically discussed error/status thread root without feeding
unrelated progress into attention. Safe failure categories distinguish final
answer decoding from uncertain backend/transport outcomes. Current reactions
and the agent's own add/remove operations are conversation-scoped; reaction
notifications are bounded active-turn observations, not approvals or idle work.
Existing Slack apps need the added reaction scopes approved and event
subscriptions updated. Source support does not imply an existing workspace has
approved those permissions.

Real WebSocket/ACP, subprocess and context fixtures cover revised silence,
interleaved chunks, replay identity, error roots, reaction deduplication and
current audience checks. General guidance distinguishes requested, delivered and
received handoffs, with a bounded context check before cancelling or duplicating
a transfer. It does not promise an atomic final read across a distributed room.
Minecraft active-turn adoption and capability changes remain owned and validated
by that adapter; these releases do not by themselves establish its live behavior.

## Recorded component checkpoint

| Component | Recorded version | Evidence and limits |
| --- | --- | --- |
| Runtime | [0.11.1](https://github.com/microsoft/amplifier-constellation-runtime/releases/tag/v0.11.1) | Native Rust execution, durable ACP sessions, ordered input, native context compaction and cache/latency observations. No Codex dependency on the native path; optional capabilities are not ACP certification. |
| Instance | [0.17.1](https://github.com/microsoft/amplifier-constellation-instance/releases/tag/v0.17.1) | Native Runtime 0.11.1 installation is the default. Negotiated source delivery, guarded maintenance and native observations. Instance and Runtime artifacts cover Linux ARM64/x64, macOS ARM64/x64 and Windows x64. |
| Observatory | [0.7.3](https://github.com/microsoft/amplifier-constellation-observatory/releases/tag/v0.7.3) | Fleet management and ACP Talk with durable send-while-busy intake, public progress, negotiated steering, queued fallback and explicit recovery. |
| Conversations | [0.8.3](https://github.com/microsoft/amplifier-constellation-conversations/releases/tag/v0.8.3) | Browser rooms, per-recipient durable intake, public progress, steering or queued fallback, Markdown, approvals and structured questions. Agent-side working folders can differ from the client launch folder. |
| Slack adapter | [0.11.0](https://github.com/microsoft/amplifier-acp-slack/releases/tag/v0.11.0) | Ordered human and active-only peer intake, negotiated source-delivery facts, public commentary/status, preserved approvals and explicit uncertain-delivery recovery. |
| Teams adapter | 0.3.0 | Local SDK, Graph, media, and ACP fixtures; live tenant/app verification remains pending. |
| Persona Workshop | 0.5.1 source | Separate communication/working-style guidance and native, AGENTS.md, and SOUL.md exports. Consistent installer naming and compatibility with existing data. Provenance and private identity remain separate. |
| ACP Lab | 0.3.2 | Repeatable scenarios, recorded participation, artifact identity, and separate executable evaluations. |
| Minecraft adapter | 0.1.0 | Independent players, scoped HTTP MCP game tools, public conversation routing, bounded tasks, and optional context/decision integrations. Requires a compatible Minecraft server; public CI uses synthetic fixtures. |

## Verification

Each component includes source tests and its own CI results. Public examples use
fictional participants. Personal installations, profiles, conversation records,
and private validation reports are excluded. Check the component's current
workflow results and platform documentation before depending on a capability.

The November 2 continuation checkpoint adds current-state attention watches,
exact predicates for host facts, receipt-based recovery advice, and deadline-aware
Minecraft scheduling. These remain optional host contracts, not ACP additions.
Predicate and model evaluations are reported separately. A 26-case transfer replay
passed: 14 exact evaluations made no provider calls, and 12 semantic evaluations
used the configured decision service (62.5 ms median transport response in this
run). This small authored suite is not a population accuracy or cost-saving claim.

The Instance dashboard now exposes acceptance briefs, delivered versions,
artifact reviews and requester closure through the same authenticated APIs.
Observatory can open the exact responsibility in that independent dashboard.
Real-service browser checks cover forms, persisted drafts, semantic control,
narrow layout and closure with a separate builder principal.

A fresh Zed GUI thread connected to the migrated native agent and used its clock
tool. Native command checks passed on a macOS ARM64 host (3 tests) and a Windows
x64 host (9 tests, including 2 child-process fixtures), in addition to Linux
checks. Windows used the explicit AppContainer feature and the serialized CI
test setup; preview limitations remain documented by the runtime.

New persona collaboration trials use actual independent ACP installations, not
chat roleplay. The shared workshop queue scenario publishes a small Python artifact
and checks its exact bytes separately over 128 input combinations. The public
example uses fictional participants; personal profiles and transcripts stay private.
Game build claims still require an independent world inspection.

## Interaction and provider checkpoint

Conversations and Observatory support negotiated agent sign-in, requester-bound
structured forms, and explicitly opened provider URLs. Observatory also offers
session history, close, resume, and deletion when the connected agent advertises
them. Session removal has a separate confirmation and does not erase an
experience's own transcript. Terminal-only login stays on the agent host.

The Slack adapter presents private dialogs to the initiating authorized person.
Questions and authentication answers are not copied into room history. Its tests
exercise the actual ACP process and Socket Mode handler against local Slack
fixtures. Teams exposes these interactions to the adapter owner through its
management dashboard; that is not a claim of native Teams participant dialogs
or a live tenant test. Answers, expiry, cancellation, and stale-session checks
remain at each experience's boundary; no agent-specific callback is required.

PDF inputs are mapped across the supported provider protocols. Live reading and
restart checks passed with OpenAI, Anthropic, and Gemini; these runs do not prove
every model supports documents. OpenAI/Azure Responses computer calls bind to
an explicitly configured MCP environment, require approval per action batch,
and preserve screenshots and uncertain outcomes across reconnects. One live
OpenAI browser task and local protocol fixtures passed. The example browser
uses a fresh profile, checks HTTP origins, refuses redirects and WebSockets,
and still requires host/network isolation for untrusted sites. Computer use is
opt-in and does not grant access to a person's existing browser or desktop.

Observe-only watch evaluation now keeps its own evidence cursor, so a later
live poll can evaluate the same state. Pending shadow evaluations cannot be
promoted into live actions. Existing watches whose shadow evidence was consumed
by an older runtime need an explicit recheck; an upgrade cannot reconstruct that
missing provenance.

## Remaining work

- Live Teams tenant/app verification is deferred; local fixtures remain available.
- Live human verification of Slack private dialogs and Teams owner-facing questions.
- Human ratings of persona fidelity and group participation; simulated ratings do not count.
- Signed/notarized distribution requires publisher credentials.
- Microphone verification on physical mobile devices remains separate from browser fixtures.
- Continue platform coverage beyond the tested macOS and Windows hosts; the runtime
  documents platform-specific constraints and preview capabilities.
- Evaluate conflict resolution, autonomous handoff, and group participation beyond
  controlled scenarios.

Production clients use ACP v1 and explicit capabilities. Draft-v2 and proposed
remote-transport prototypes remain isolated experiments. The Instance network
bridge is a documented custom transport behind a standard stdio attach entry
point. Neither prototypes nor custom transport URLs imply universal host support.

## November 4 release checkpoint

Instance 0.15.1 owns optional pre-turn social evaluation, credentials, budgets,
source freshness, deduplication and one-use admission offers. Slack 0.8.0 and
Conversations 0.7.0 support this optional profile. The corresponding Minecraft
change remains independently reviewed by its adapter owner; do not enable two
social evaluators for the same event. Task-condition watches are a separate concern.
Ordinary ACP remains usable without this profile or a decision-model account.

Native and Codex engines completed bounded live compatibility checks. Adapter
fixtures verify that a held event sends no prompt and an offered turn skips the
previous LLM-only social preflight. A small authored JEV calibration distinguished
low-confidence choices from explicit uncertainty. This is not a production
accuracy or savings estimate. AHP remains a future host/client option.

Runtime 0.9.0 accepts supported MCP image results and provides separate workspace
image-view and image-attachment tools. Reading an image does not automatically
publish it. Size/type limits, authorization, attribution and replay handling are
covered by protocol tests; provider/model support remains negotiated.

Slack 0.8.2 ignores parent thread-bookkeeping events with unchanged content,
so reply counts do not masquerade as edits of the initiating request. It also
preserves active tool permissions across teammate replies;
new human instructions and changed initiating sources still require reassessment.
Both automatic and human-decision paths have regression coverage.

That is the historical 0.8.2 checkpoint. Slack 0.9.0 also preserves ongoing work
and unchanged approvals across ordinary human follow-ups. True source retraction,
explicit stop, changed authority or a materially changed operation remain reasons
for reevaluation under the input-continuity contract.

Slack recovery covers missed human messages and replies in known or newly
discovered threads, with bounded pagination and retained checkpoints. Unknown old
thread roots can remain undiscoverable. Recovery does not replay already accepted
or completed effects. Process fixtures cover offline delivery, an old-root thread,
explicit cancellation, interrupted work and restart deduplication.

The independently owned update helper supports frozen, hashed provider plans,
verification and rollback. The Linux container provider can replace an existing
enrolled container with an owner-selected immutable image while preserving its
manifest and keeping the previous container for recovery. A real image-swap and rollback check restored the original container, manifest and
agent identity. Version 0.15.1 waits for container startup before checking rollback
health; the independently enrolled provider must also be updated from that source.
This is not the broader shared-environment product. Source checkouts, Minecraft installations and unrelated
programs require their own owner-enrolled provider; an immutable image reference
alone is not an automatically published image feed.

Release checks passed on the documented CI platforms. Updates were also verified
on operator-managed Linux and macOS installations while preserving native versus
Codex engine choices. These are bounded deployment checks, not proof of every
user's installation. Personal deployment details and transcripts remain private.

## Next experience proposals

[Shared spaces](shared-spaces-design.md) describes an outcome-first consumer
journey using the existing independent controls. Its interactive concept is a
design aid, not a connected product. [Execution environments](execution-environments.md)
remains design-only: separate working copies by default, optional shared folders,
and distinct identities and credentials. Neither proposal introduces a mandatory
shared provisioning or credential service.

## End-to-end delivery exercise

A real Slack assignment produced the [meeting availability example](https://github.com/microsoft/amplifier-acp-lab/pull/2)
through independent native and Codex agents. The builder handled an interruption,
a changed duration requirement and a later CI request within the same checkout
and PR. Another agent reviewed the exact candidate and ran its tests plus
separate counterexamples; a coordinator requested acceptance only after that
evidence existed. The reviewed result was accepted and merged.

The final candidate `0f08d95ec1675b69329e5c19c5be3bcf92b1009d` has 13 example
tests in Python 3.11/3.13 CI. Its unchanged implementation also passed a separate
1,000-case minute-grid comparison. Operator interventions were required to fix
the Slack permission and thread-metadata defects, change the requirement, request
CI coverage, and accept the result. This is a bounded delivery demonstration, not
zero-intervention autonomy or a human persona fidelity evaluation.

The reply collectors in Instance 0.16.3, Slack 0.10.1, Conversations 0.8.2 and
Observatory 0.7.2 ignore empty control text before message ordering and phase
classification. Metadata receipts continue to be processed independently.
