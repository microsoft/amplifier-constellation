# Harness lifecycle and working context (candidate v1)

The durable conversation is evidence. The model working window is a provider-specific view of that evidence. Compaction never edits the original transcript or grants new authority. Preserve provider-signed/opaque continuation blocks exactly; keep them out of user-visible conversation and activity. Observe actual input, cached input, output, compaction usage and elapsed time separately. Bytes are not token counts or cost savings.

## Work, transport and maintenance

An assignment deadline, tool-operation deadline, connection inactivity limit and model turn are separate clocks. Healthy long work must not fail merely because a short adapter timer elapsed. Task owners still enforce their original deadlines and action budgets. A silent standard ACP peer has unknown progress, not proof of failure. A connection close, cancellation request or accepted maintenance request is not successful terminal settlement. Do not replay possibly accepted work.

Use standard ACP `clientCapabilities.session.compaction` and `compaction_update` when supported. A completed compaction update does not itself settle `session/prompt`. Instances may preserve the exact advertised `/compact` command before prompt decoration. An opt-in instance capability under `io.github.bkrabach.agent-instance.maintenance` describes this route: `version:1`, `promptMetadata:"com.microsoft.amplifier.maintenance"`, `requiresAdvertisedCommand:true`. Clients send one `/compact` text block in `session/prompt`, with that metadata containing `version:1`, `operation:"compact"`, and a stable `operationId`. The session must be idle across all connections, and the operation ID cannot be reused. Query `_amplifier/maintenance_receipt` with `sessionId` and `operationId` to inspect settlement after a lost response. The Instance prevents competing input and denies effectful client callbacks until the matching prompt settles. A first `completed` compaction update is valid without an earlier `in_progress` update; a previously observed compaction ID cannot settle a new operation. The result metadata reports completed only after a matching successful compaction update AND successful prompt terminal; other outcomes remain failed/cancelled/unknown. A backend owns its own internal execution; this profile is not a general sandbox for arbitrary third-party implementations.

Adapters close their action authority before maintenance, never reserve a work action lease for it, and recheck existing assignment authority afterward. Native Runtime compacts its provider window automatically; it does not pretend to implement a backend command that it does not advertise.

## Source delivery facts

Instances may advertise `inputContinuity.sourceDelivery` version 1 with metadata `com.microsoft.amplifier.source-delivery` and receipts method `_amplifier/source_receipts`. This is optional, not standard ACP. On steering input the metadata contains `version:1` and up to 40 `sources`, each with `id`, opaque `revision`, and original `occurredAt` (milliseconds). The normal stable steering input ID and active-prompt/session binding still apply. Source IDs and revisions belong to the connector; Instance binds them to the authenticated session and audience.

Attention `context.messages[]` may include both `revision` and `occurred_at` (snake_case, original milliseconds) only when this capability is negotiated. Steering and receipt-query sources instead use `occurredAt` (camelCase). Instance adds its observed delivery facts to attention evidence only for matching source ID, revision and original time. A changed revision or reminder must not reuse an old delivery fact. A receipt query takes `sessionId` and the same bounded `sources`. Results distinguish received, submitted, remoteQueued, incorporated, notAccepted, promptRequired and unknown. No adapter-supplied claim may manufacture incorporation. Missing receipt capability or an ambiguous transport outcome remains unknown. Receipt evidence is not task completion, public delivery, or permission; it does not automatically suppress another prompt. A new source revision, new question or reminder remains eligible for social judgment. Preserve final-answer freshness checks.

## Presentation

Show only public commentary (`agent_message_chunk` with explicit commentary phase), public tool/status fields, and negotiated compaction status. Never expose private thought chunks, opaque compaction state, tool arguments or results as a status label. Prefer a single concise, changing line in the responding conversation, throttled to the platform. Thinking, compacting, working, waiting and failure are distinct observations. Commentary is not the final answer and must not replace one. Silence decisions remain silent.

## Measurement and rollout

Expose model-step counts, provider response counts, reported usage/cache tokens,
context bytes and maintenance time as separate measurements with coverage.
Missing samples are unknown. Smaller prompts and cache hits alone do not prove
lower total cost or better outcomes. Compare matched tasks, effects, latency,
context growth, duplicate activations and failure rates before claiming gains.

Native foreground compaction is the initial supported baseline. Background
compaction needs captured-prefix/settings validation plus a preserved suffix;
never apply a summary to a different conversation revision. Installation
configuration, CLI and UI must expose equivalent controls and observations.

Guidance: [ACP prompt turns](https://agentclientprotocol.com/protocol/v1/prompt-turn),
[OpenAI compaction](https://developers.openai.com/api/docs/guides/compaction),
[OpenAI cache diagnostics](https://developers.openai.com/api/docs/guides/prompt-caching/diagnostics),
[Anthropic compaction](https://platform.claude.com/docs/en/build-with-claude/compaction-on-demand),
[context engineering](https://www.anthropic.com/research/effective-context-engineering-for-ai-agents).
