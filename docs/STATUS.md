# Implementation status

November 4, 2026. This is a dated development checkpoint, separate from the vision
and draft umbrella contracts. Downloadable releases are listed by each component. Source-only components are
identified below. Release availability is not an assertion that every installation
has been updated.

## Current components

| Component | Current version | Evidence and limits |
| --- | --- | --- |
| Runtime | [0.9.0](https://github.com/microsoft/amplifier-constellation-runtime/releases/tag/v0.9.0) | Native Rust model/tool execution and durable ACP sessions; no Codex dependency on the native path. Optional capabilities and trust profiles are documented in the runtime repository; this is not official ACP certification. |
| Instance | [0.15.1](https://github.com/microsoft/amplifier-constellation-instance/releases/tag/v0.15.1) | Native installation is the default. Artifacts and installation checks cover Linux ARM64/x64, macOS ARM64/x64, and Windows x64. Checkpoint export/restore/clone and authenticated completion APIs have separate validation. |
| Observatory | 0.6.0 | Fleet and machine management, optional management-provider discovery, ordinary ACP Talk, and experiment reports. |
| Conversations | [0.7.0](https://github.com/microsoft/amplifier-constellation-conversations/releases/tag/v0.7.0) | Browser rooms, direct conversations, Markdown, approvals, and negotiated structured form questions. Agent-side working folders can differ from the client launch folder. |
| Slack adapter | [0.8.2](https://github.com/microsoft/amplifier-acp-slack/releases/tag/v0.8.2) | Standard ACP interaction, Slack formatting and delivery, scoped context, and explicit callback approval. |
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
