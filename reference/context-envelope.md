# Refreshed context envelope v1

Optional shared producer envelope for refreshed client context. This preserves
its precise metadata bounds; behavioral obligations belong to
[work continuity](../contracts/work-continuity.md), and model-view selection,
unsupported-input fallback and implementation tests belong to
[Runtime](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/architecture.md#refreshed-client-context).
No extension is required of ordinary ACP clients or engines.

The consumer advertises `_meta["com.microsoft.amplifier.context-snapshot"]` in
initialize with `version: 1`, `contentBlocks: true` and
`originalHistoryPreserved: true`. Producers use it only after negotiation;
intermediaries preserve content-block metadata when prepending context.

A text content block may carry that namespace with:

| Field | Constraint |
| --- | --- |
| `version` | `1` |
| `key` | Nonempty, at most 256 bytes, publisher/conversation/audience-specific. Distinct sources or audiences use distinct keys. |
| `retainedText` | Nonempty, at most 32 KiB; contains the historical request and essential continuation state. |

A one-time instruction must not exist solely in the replaceable snapshot. Media
and independent instructions remain separate blocks. Selection retains the newest
valid snapshot per key within a session; older snapshots retain position and
retained text. Full original content and ACP blocks remain canonical for replay
and authorized retrieval; archive hashes refer to those originals. Invalid or
unsupported metadata leaves content unchanged.

Verify boundary lengths, distinct audience keys, mixed media, malformed metadata,
original archive retrieval and tool pairing. This envelope neither authorizes
retrieval nor proves source freshness or external effects.
