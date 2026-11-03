# Implementation status

November 2, 2026. This is a dated development checkpoint, separate from the vision
and draft umbrella contracts. These are reviewed release candidates; downloadable releases are listed by each component.
A candidate is not an assertion that every installation has been updated.

## Current components

| Component | Current version | Evidence and limits |
| --- | --- | --- |
| Runtime | 0.8.0 | Native Rust model/tool execution and durable ACP sessions; no Codex dependency on the native path. Optional capabilities and trust profiles are documented in the runtime repository; this is not official ACP certification. |
| Instance | 0.12.0 | Native installation is the default. Artifacts and installation checks cover Linux ARM64/x64, macOS ARM64, and Windows x64. Checkpoint export/restore/clone and authenticated completion APIs have separate validation. |
| Observatory | 0.5.0 | Fleet and machine management, optional management-provider discovery, ordinary ACP Talk, and experiment reports. |
| Conversations | 0.6.0 | Browser rooms, direct conversations, Markdown, approvals, and negotiated structured form questions. Agent-side working folders can differ from the client launch folder. |
| Slack adapter | 0.6.0 | Standard ACP interaction, Slack formatting and delivery, scoped context, and explicit callback approval. |
| Teams adapter | 0.3.0 | Local SDK, Graph, media, and ACP fixtures; live tenant/app verification remains pending. |
| Persona Workshop | 0.5.1 | Separate communication/working-style guidance and native, AGENTS.md, and SOUL.md exports. Consistent installer naming and compatibility with existing data. Provenance and private identity remain separate. |
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
