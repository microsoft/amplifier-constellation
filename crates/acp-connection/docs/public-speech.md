# Public speech assembly and completion profile v1

Technical agreement for ACP producers, proxies and consumers using the shared
[`speech` collector](../src/speech.rs). Human-facing promises live in
[Interaction](../../../contracts/interaction.md). This profile is optional;
ordinary ACP peers remain supported.

## Message identity and phase

ACP v1 `agent_message_chunk` carries streamed content; optional `messageId` groups
chunks. Preserve identities through proxying and session replay. Interleaved
chunks retain their message, and a new identity does not truncate or complete an
older one. Without identity, preserve supported legacy append behavior.

Where available, producers expose
`_meta["com.microsoft.amplifier.message"].phase` as `commentary` or `final_answer`.
Private reasoning is never commentary. Final-answer selection chooses the latest
non-commentary answer in message creation order, retaining separate intermediate
messages and tool history. A silence decision can replace an unpublished draft;
it cannot retract speech already delivered. Keep media separately bounded rather
than dropping it when text identity changes.

Empty text updates may carry input receipts or other controls. Process the
metadata without creating a message, changing phase or order, consuming an answer
slot, inserting text separators or firing assistant-text callbacks. Empty updates
must not change selection of later nonempty text.

## Optional completion negotiation

Stable ACP v1 has no completed-message event. A producer may advertise
`_meta["com.microsoft.amplifier.message"].completion = {"version": 1}` in its
initialize result. A client opts in with the same value under
`clientCapabilities._meta`. Proxies preserve negotiation; they cannot invent
completion for an opaque backend.

After confirmed provider completion, the producer may send an empty text
`agent_message_chunk` with the original `messageId` and metadata. For accumulated text `Hello.`:

```json
{
  "com.microsoft.amplifier.message": {
    "completed": {
      "version": 1,
      "utf8Bytes": 6,
      "sha256": "2d8bd7d9bb5f85ba643f0110d50cb506a1fe439e769a22503193ea6046bb87f7"
    }
  }
}
```

Use the lowercase SHA-256 of the exact accumulated UTF-8 text. The marker settles
only an existing draft whose byte length and digest match. It creates no text,
identity, order slot or phase. Duplicate matching completion is harmless;
appending to a completed message is an error, never permission to edit published
text. The digest checks assembly, not trust or authority.

The current integration applies early completion to explicitly public
`commentary`. Final answers keep the latest unpublished revision until successful
prompt settlement, then post as new messages. Completed commentary can remain
delivered if subsequent work fails; failure never settles unfinished text.
Without this signal, known public commentary settles only at a successful prompt
terminal. Private thought, tool results and participation-control output never
become speech. A network frame, punctuation mark, timeout or cancellation is not
a completion boundary.

## Producer mapping and delivery

OpenAI Responses maps completed assistant messages from `response.output_item.done`,
checking the item's status. Other native provider paths may settle text at their
successful message/response terminal before tool execution. Preserve the distinction
between a completed contribution and completed work. Each producer documents and
tests the boundaries it actually provides.

The shared collector uses upstream SDK typed messages and reserved metadata.
Adapters own formatting, posting and status controls. Persist delivery identity
before posting; distinguish prepared, attempted, delivered and uncertain outcomes.
Reconcile lost replies through the surface's supported identity/history lookup,
preserving original audience and thread. Include successful public posts in
accessible source history; transient status stays separate. Do not replay effects
to repair text delivery or formatting failures.

## Compatibility checks

Exercise interleaved identities, partial chunks, duplicate completion, mismatched
hash/length, empty receipts, phase changes, success/failure/cancellation and legacy
fallback. Check actual append order across two agents and a person, unknown post
recovery, retained media and absence of private/control content. Assembly tests
are in [`speech.rs`](../src/speech.rs); platform delivery needs its own tests.

Keep diagnostics bounded to identities, stages and outcomes, without raw private
prompts or credentials. Reassess this augmentation when upstream ACP/SDK supplies
a usable message lifecycle; remove it once negotiated upstream behavior covers
these cases.

References: [ACP prompt lifecycle](https://agentclientprotocol.com/protocol/v1/prompt-turn),
[ContentChunk](https://agentclientprotocol.com/protocol/v1/schema#contentchunk),
[OpenAI streaming](https://developers.openai.com/api/reference/resources/responses/streaming-events),
[Anthropic streaming](https://platform.claude.com/docs/en/build-with-claude/streaming).
