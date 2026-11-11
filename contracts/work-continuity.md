# Continuing and recovering work

For source adapters, Instance, engines and domain operations. The promise: new
input, long work and interruption preserve useful progress, source meaning and
honest outcomes. Supported engine capabilities determine the available path;
this agreement does not add capabilities to an opaque backend.

**Example:** A person corrects a briefing while an agent is collecting evidence.
The correction reaches the same work without repeating completed actions. If the
connection drops, the agent reconciles delivery and effects before continuing.

## Commitments

1. **Keep each input's meaning and owner.** Preserve source ID/revision, author,
   audience, original time and reliable intake order. Deduplicate a source
   revision, not equal text. Edits are corrections, not rewritten provider history;
   queued edits retain their revision audit. Historical retrieval, approvals,
   status cards and an agent's output are not fresh participant instructions.
   Cross-room input requires explicit audience/context policy. Sources own intake;
   Instance owns eligibility/routing; engines own accepted-input continuity.

2. **Acknowledge only durable ownership.** An accepted input belongs to exactly one
   lifecycle, including races with finalization and reconnect. Preserve distinct
   received, submitted, remotely queued, incorporated, not-accepted and unknown
   outcomes; incorporation means included in context, not understood or completed.
   Queue capacity rejects unaccepted input explicitly; it cannot silently drop
   acknowledged input. Adapters retain source evidence until ownership/delivery
   is reconciled. An earlier unknown must not freeze unrelated authorized new
   requests, but cannot authorize replay of related uncertain effects.

3. **Continue rather than restart.** New guidance does not cancel work, erase
   results, dismiss unchanged approvals or create a task per message. Use
   negotiated steering or retain ordered input for the next supported boundary.
   Opaque non-steering engines finish their current prompt first; never simulate
   steering by cancel-and-reprompt. Batches preserve every author, boundary and
   revision, bounded by count/bytes; no indefinitely resetting debounce or silent
   skipped attachment. Unsupported input has an explicit outcome. There is one
   controlling prompt lifecycle per saved session; active-only awareness rejected
   as idle cannot become a new prompt or acquire another admission offer. Independent agents retain
   independent sessions, credentials and budgets.

4. **Finish with admitted guidance accounted for.** Serialize admission with
   closure so admitted input belongs either to the current turn or the next one. A
   final-looking answer remains intermediate while admitted input is pending;
   continue from saved output/effects. Do not erase already published speech.
   Provider continuations retain the original limits, including model/tool calls,
   deadlines and cost. Exhaustion retains pending input and reports why work
   stopped. Busy conversations cannot renew tasks or starve other sessions.

5. **Plan ahead without claiming premature effects.** Existing domain mechanisms
   may accept interruptible sequences and return a durable operation ID while
   execution continues. Accepted/running is not completed. Status and cancellation
   use that same identity and authority. Recheck authority and resource conditions
   before every step; Stop, retraction, changed conditions and failed dependencies
   prevent unstarted steps. Settle started operations under their cancellation
   contract, preserving partial/unknown results and charging each actual effect.
   Resource owners serialize incompatible operations. Lost replies never justify
   sequence replay. Missing optional model-written progress reports cannot erase
   successful effects or masquerade as capacity waits; identify any real review
   prerequisite. Valid work may continue without repeated nudges; completed,
   stalled or impossible work does not justify empty planner loops.

6. **Preserve evidence while reducing context.** Original transcripts, requests,
   tool results and opaque provider continuation state remain canonical. Compaction
   and refreshed snapshots change the model view only, preserving essential
   unfinished work, source coverage, order and tool pairing. Incremental updates
   avoid resending whole histories. Do not reset a live provider chain awaiting
   results or hide new instructions in tool outputs. Snapshot keys remain source/
   audience-specific; retrieved context grants no authority. Invalid or unsupported
   projections retain original meaning. Keep current installation identity, scope
   and source coverage. Background summaries require matching captured history/
   settings and preservation of later input. Runtime owns [projection details](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/architecture.md#refreshed-client-context)
   and [provider sequencing](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md).

7. **Reconcile uncertainty under current authority.** Transport close, cancellation
   request, silence or maintenance acceptance is not successful settlement. Work,
   tool, model and connection clocks are separate; a short transport timer must
   not redefine a task deadline. Preserve original sessions, results and allowances.
   Retry needs verified non-delivery or reconciliation, not an ambiguous failure.
   Supported restored attention tickets retain source, expiry, audience and spent
   slot; explicit restoration and fresh authorization are required. Ordinary offers
   do not become portable. See [Instance's admission boundary](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/attention.md).
   Receipt/recovery controls require current loaded-session, conversation and tool
   authorization; revocation applies after load too.

8. **Keep maintenance and observation truthful.** Supported maintenance runs with
   action authority closed and no work lease; require matching completion before
   rechecking existing authority. Optional [maintenance/source receipts](../reference/maintenance-and-source-receipts.md)
   add evidence, not tasks or replay rights. Keep activity separate from public
   speech and private reasoning. Report input state, linkage, retries and actual
   progress without exposing raw content or credentials. Distinguish bytes, tokens,
   cache usage, cost, maintenance time and latency; missing samples remain unknown.
   Compare matched outcomes before claiming improvement.

**Check:** Exercise bursts during model calls, tools and approvals; input racing
completion; duplicate source/provider events; disconnect before/after acceptance;
Stop and revoked access; non-steering fallback; lost async-operation acknowledgments;
and compaction/reload with original history retrieval. Count actual effects and
records, not merely replies. Check order, audience, unchanged authority, bounded
resources, successful effects despite a missing optional report, and honest unknowns.

**Left open:** Storage, queue implementation, provider transport and domain sequence
API. Exact optional protocols live with their serving owners; no universal inbox,
background compactor, history format or atomic planning capability is implied.
