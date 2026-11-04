# Refreshed context snapshots — version 1 candidate

This optional extension reduces repeated source projections while preserving
requests and durable history. Ordinary ACP clients and agents remain valid.
It does not merge conversations or grant access.

## Producer and consumer

The runtime advertises `_meta["com.microsoft.amplifier.context-snapshot"]` in
initialize with `version: 1`, `contentBlocks: true`, and
`originalHistoryPreserved: true`. A client uses the extension only after this
advertisement. Intermediaries preserve block metadata when prepending context.

A text content block may carry the same namespace with:

- `version: 1`.
- `key`: a nonempty publisher/conversation/audience-specific identifier, at most
  256 bytes. Different sources or audiences must use different keys.
- `retainedText`: the historical user request and essential continuation state,
  nonempty and at most 32 KiB. A one-time instruction cannot exist solely in the
  replaceable snapshot. Media and independent instructions stay separate blocks.

The consumer selects the newest valid snapshot for each key within one session.
Older snapshots retain their position and retained text. Other user content,
assistant output, tool calls/results and opaque provider state remain ordered.
Unknown versions, invalid metadata and unsupported content conversions remain
unchanged. Ordinary context-budget compaction can follow this selection.

## Durable evidence and authority

Full original content and ACP blocks remain saved and available through replay
and authorized history retrieval. Archive indexes/hashes refer to those originals.
Snapshot selection changes the model view only. It never grants permissions,
widens an audience, alters freshness checks, proves an external side effect, or
converts retrieved evidence into a new instruction.

Installation context has its own slot. Current identity/profile, scope, memory,
responsibilities and source coverage remain present. Experiences own their
source projections; runtimes do not parse platform-specific transcript strings.

## Interruption and delivery guidance

A follow-up question need not abandon previously accepted work. Experiences
should carry a bounded account of unfinished work into its successor while
retaining current-source permission checks. Explicit stops, edited/deleted
sources, changed scope, and actual rejections still apply. Completed effects
must be verified before retry; lifecycle events alone do not prove success.
Undelivered drafts must remain distinguishable from messages the audience saw.

Optional permission diagnostics may use
`_meta["com.microsoft.amplifier.permission"].reason` with `source_changed`,
`policy_changed`, or `expired`. The standard ACP outcome remains authoritative.
A reason code never changes cancellation into approval. Consumers should not
mistake stale-turn cancellation for a remote account or repository permission
failure, or echo arbitrary diagnostic text as trusted authority.

## Validation

Exercise restart/resume, decision-to-response transitions, mixed media, malformed
metadata, distinct audience keys, original archive retrieval, tool pairing, and
explicit cancellation. Measure provider input with the original history-selection
policy visible; savings estimates from a fixed tail are not measurements of a
new model call. Keep private transcripts and deployment evidence outside public
repositories.
