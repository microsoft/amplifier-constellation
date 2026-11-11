# Working together without wasting attention

For experience, connector, conversation-service and agent owners. The promise:
people can direct and inspect useful collaboration without following every agent
exchange, and meaningful results return to where they belong.

**Example:** A person asks a group for a recommendation. The agents coordinate in
an inspectable huddle, incorporate a correction and bring back one useful answer,
including disagreements and decisions still needed. The person can join the
work without becoming its message relay.

## Commitments

1. **Keep the conversation's identity.** Preserve the speaker, source, audience,
   thread and return destination. A late response does not follow a newly selected
   screen into another room. Recognizing the same agent across experiences does
   not merge private conversations. Sources retain access rules under
   [composition](composition.md); retrieved material does not expand them.

2. **Treat human attention as valuable.** Normally respond to direct or explicitly
   addressed input. Ambient conversation may warrant participation or silence;
   silence produces no public “nothing to add” message or working indicator.
   Fresh, source-attributed human messages must not be suppressed or charged by
   quotas intended to limit ambient chatter. Another addressee can still warrant
   silence. Actual service limits and permissions remain meaningful constraints.
   Explain a relevant blocker briefly without mislabeling its cause.

3. **Use one participation decision.** Instance owns optional shared attention
   policy; connectors supply attributed source facts and perform supported routing.
   They do not add a second social classifier. Unknown actor kind is not proof of
   a human, and a peer cannot obtain human priority by quoting one. Recent actual
   activity can be relevant evidence; idle presence, repeated unchanged samples
   and the agent's own output are not reasons to keep it awake. Activity is not
   speech or an instruction. Ordinary compatible ACP conversation remains possible
   without this optional policy. Reactions remain scoped social evidence; they
   neither approve tools nor launch idle work.

4. **Keep coordination quiet and available.** Reuse an appropriate authorized
   conversation before creating a huddle. People can inspect without prompting
   an agent and join when useful. Participation, access, notification and work
   authority remain distinct. Preserve lively multi-agent brainstorming when
   people are working with the group. Use existing delivery and participation
   mechanisms rather than a new quota or classifier. If a platform lacks a quiet
   destination, document its notification limits and offer a supported fallback;
   never claim hidden coordination or flood the human channel. An independently
   configured Conversations service can provide that optional fallback.

5. **Close the loop.** For a shared result, establish who accepts responsibility
   for reporting it. Nomination, acceptance and delivery are different. Preserve
   the expected result, important objections and reporting state in concise,
   source-linked records. Bring meaningful progress, changed commitments, results
   and decisions needing judgment back to the original authorized destination.
   An accepted reporting duty is not optional ambient news and needs no repeated
   permission to report. Others add material corrections rather than duplicate
   the recap. A proposal or report grants no new work authority; see
   [work authority](work-authority.md). Resource transfers distinguish requested,
   offered, deposited and received; a sender receipt alone cannot prove recipient
   possession. Receipt, execution, delivery, artifact verification and requester
   acceptance remain separate observations.

6. **Distinguish activity from speech.** Working indicators are temporary, scoped
   to the response destination and cleared or reconciled on settlement. Show an
   informative public operation label when available, with Thinking… for model
   wait and an honest indication of delivery or human wait. A global presence
   flag cannot substitute for room-specific status. Use the richest supported
   affordance without status-message spam; omit intermediate indicators if none
   is suitable. Concurrent turns and tools retain independent activity; one
   completion cannot clear another. Never expose private reasoning, arguments,
   credentials or another audience's contents. Labels are observations, not completion evidence.

7. **Publish complete contributions.** Speech is a permanent participant message;
   subsequent commentary, corrections and final answers append in observable
   order rather than overwrite earlier speech or inherit a working indicator's
   old timestamp. Chunks and pauses are not utterance boundaries. Use negotiated
   completion information, or retain drafts until a successful prompt terminal.
   Failed streams do not turn unfinished drafts into answers. Delivery keeps a
   durable identity and reconciles uncertainty without blindly reposting.

8. **Keep useful context, remove control noise.** Progress, approval controls,
   own-output echoes and connector notices are not new participant input or peer
   activation. Filter structured control events before bounded context limits,
   using metadata and owned IDs, never text matching. Keep them visible to people.
   If a person asks about a notice, retain that exact visible root as a labelled,
   authorized reference without its private control data. Handle new conversation
   input through [work continuity](work-continuity.md); receipt is not incorporation
   or fulfillment. A genuine contribution must not disappear because it resembles
   a status label.

9. **Make controls accessible.** UI, voice, CLI and authorized agents reach the
   same service operations and permission checks. Meaningful navigation, selection,
   drafts, focus and view state have documented programmatic access, with revision
   checks protecting newer intent. Count setup, recovery and administrator effort
   alongside user effort; reuse known configuration where permitted.

## Check the combined experience

Observe people and agents making a decision, coordinating quietly, joining and
unfollowing a huddle, correcting work and returning the result. Judge interruption,
useful contribution, missed decisions and comprehension—not just transport success.
Test wrong-audience late replies, exhausted ambient quotas, status-like human text,
parallel tools, failed streams and uncertain posts. Verify one useful failure
presentation, no status-triggered agent loops, no duplicate speech and an honest
fallback without optional capabilities. Compare equivalent UI/API controls and
reject stale view updates. Check actual outcomes separately from what agents claim.

## Interface owners and open choices

[Instance attention](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/attention.md)
owns optional admission schemas, budgets and ticket handling.
[Conversation services](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/huddles.md)
own huddle operations. The [shared speech collector](../crates/acp-connection/docs/public-speech.md)
owns message assembly and its optional completion profile. Connectors own platform
presentation and delivery; engines own execution and input handling.

Product layouts, notification preferences and coordination strategies remain open.
This agreement requires neither a central leader-election service nor identical
controls across platforms. Freshness remains limited to evidence actually
available at the supported input boundary; do not infer an atomic final check across rooms or start unbounded
reconsideration.
