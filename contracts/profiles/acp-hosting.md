# Hosting a selected ACP engine

For Constellation Instance, its connectors and hosted ACP engines. This is the
supplied hosting design, not a required [Agent Instance](../agent-instance.md)
architecture. Optional features apply when negotiated; ordinary ACP conversation
remains usable without them. [Engine compatibility](../engine-compatibility.md)
owns protocol integration; [interaction](../interaction.md),
[authority](../work-authority.md) and [continuity](../work-continuity.md) own
shared behavior.

## Participation

Instance owns the optional participation decision; connectors supply attributed
facts and routing without a second social classifier. Unknown actors and quoted
humans confer no human priority. Recent actual activity may inform participation;
idle presence, unchanged samples and own output do not keep an agent awake.
Activity is not speech or instruction; reactions neither approve tools nor launch
idle work. Available evidence bounds freshness, without an atomic cross-room
check or unbounded reconsideration.

Sources own intake, Instance eligibility/routing, and engines their execution
history and accepted-input continuity. [Instance attention](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/attention.md)
owns schemas, budgets and tickets; interaction owns control-event filtering.

## Saved sessions and input

Each saved execution session has one controlling prompt lifecycle. Independent
agents keep independent sessions, credentials and budgets; an Agent Instance may
contain multiple sessions and workers.

Non-steering engines finish their current prompt before receiving queued input;
do not imitate steering by cancel-and-reprompt. Input batches preserve authors,
source boundaries and revisions within count/byte bounds, without indefinitely
resetting debounce or silently skipped attachments. Active-only awareness rejected
as idle cannot become a fresh prompt or obtain another admission offer.

Serialize admission with closure so admitted input belongs to the current turn
or the next. While input is pending, treat a final-looking answer as intermediate
and continue from saved output/effects under the continuity agreement. See
[conversation continuity](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/conversation-continuity.md)
and [engine steering](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/steering-design.md).

Publish using negotiated speech-completion information; otherwise retain drafts
until a successful prompt terminal. The [shared collector](../../crates/acp-connection/docs/public-speech.md)
owns message assembly and the optional completion profile.

## Context projections

Original transcripts, requests, tool results and opaque provider continuation
state remain canonical. Compaction/snapshots change only the model view, retaining
unfinished work, source coverage, order and tool pairing. Use incremental updates;
never reset a provider chain awaiting results or conceal instructions in tool
outputs. Snapshot keys are source/audience-specific; retain installation identity
and scope. Invalid or unsupported projections preserve original meaning. Background summaries
must match captured history/settings and preserve later input. Runtime owns
[projection fields and processing](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/architecture.md#refreshed-client-context).

## Restoration and maintenance

Supported attention-ticket restoration retains source, expiry, audience and spent
slot, with explicit restoration and fresh authorization. Ordinary offers are not
portable. Receipt/recovery controls check the currently loaded session,
conversation and tool authorization under the shared authority agreement.

Maintenance runs with action authority closed and no work lease. Match its
completion before rechecking existing authority. Optional
[maintenance/source receipts](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/harness.md)
are evidence, not tasks or replay rights.

Optional [watch](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/watches.md)
and [continuation](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/continuation.md)
helpers require compatible bound discovery. Watches retain finite TTL, identity,
delivered evidence and accounting.

## Execution resources

Use separate working copies by default; shared writable folders require explicit
choice. Execution boundaries may host one or several agents. Restrict host
filesystem access while retaining Internet access and external authorization;
[composition](../composition.md) governs shared-resource access.

**Check:** Exercise tool-time input, closure races, idle-awareness rejection,
non-steering fallback, interrupted maintenance and revoked access after load.
Compact/reload while retrieving original history. Verify session ownership,
source order, complete tool pairs and the shared work guarantees. Storage, queues
and worker organization remain open.
