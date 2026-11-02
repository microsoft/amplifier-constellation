# Implementation status

November 2, 2026. This is a dated development checkpoint, separate from the vision
and draft umbrella contracts. These are source versions; downloadable releases are listed by each component.

## Current components

| Component | Current version | Evidence and limits |
| --- | --- | --- |
| Runtime | 0.7.0 | Native Rust model/tool execution and durable ACP sessions; no Codex dependency on the native path. Optional capabilities and trust profiles are documented in the runtime repository; this is not official ACP certification. |
| Instance | 0.11.0 | Native installation is the default. Artifacts and installation checks cover Linux ARM64/x64, macOS ARM64, and Windows x64. Checkpoint export/restore/clone and authenticated completion APIs have separate validation. |
| Observatory | 0.4.0 | Fleet and machine management, optional management-provider discovery, ordinary ACP Talk, and experiment reports. |
| Conversations | 0.5.1 | Browser rooms, direct conversations, Markdown, approvals, and negotiated structured form questions. Agent-side working folders can differ from the client launch folder. |
| Slack adapter | 0.5.0 | Standard ACP interaction, Slack formatting and delivery, scoped context, and explicit callback approval. |
| Teams adapter | 0.2.1 | Local SDK, Graph, media, and ACP fixtures; live tenant/app verification remains pending. |
| Persona Workshop | 0.5.0 | Separate communication/working-style guidance and native, AGENTS.md, and SOUL.md exports. Provenance and private identity remain separate. |
| ACP Lab | 0.3.2 | Repeatable scenarios, recorded participation, artifact identity, and separate executable evaluations. |

## Verification

Each component includes source tests and its own CI results. Public examples use
fictional participants. Personal installations, profiles, conversation records,
and private validation reports are excluded. Check the component's current
workflow results and platform documentation before depending on a capability.

## Remaining work

- Verify Teams against a configured live tenant, app identity, and HTTPS callback.
- Complete a fresh Zed GUI conversation check against the migrated native agent.
- Add interactive provider sign-in where needed; owned clients currently offer
  a preauthenticated fallback.
- Broaden optional callbacks and session controls per experience. Only Conversations
  currently advertises structured form elicitation among the owned interactive UIs.
- Broaden native command-isolation validation on macOS and Windows; the runtime
  documents platform-specific constraints and preview capabilities.
- Add dedicated UI controls for the completion contracts already exposed by API.
- Evaluate conflict resolution, autonomous handoff, and group participation beyond
  controlled scenarios.

Production clients use ACP v1 and explicit capabilities. Draft-v2 and proposed
remote-transport prototypes remain isolated experiments. The Instance network
bridge is a documented custom transport behind a standard stdio attach entry
point. Neither prototypes nor custom transport URLs imply universal host support.
