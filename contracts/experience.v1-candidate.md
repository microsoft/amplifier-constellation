# Interaction across services and experiences — v1

For experiences, connectors, agent services and management clients. The promise:
a scoped request retains its meaning through execution and return. Product UX
and platform presentation remain with their owners.

## Promises

1. **Preserve origin and destination.** Keep actor, conversation, audience, thread
   and destination identifiable. A late response cannot follow a changed screen
   selection into another conversation.
2. **Respect source access.** Context, persona and recognized identity grant no
   access. Current source grants and destination membership govern retrieval and
   sharing; the same individual does not imply one transcript.
3. **Offer equivalent controls.** UI, voice, CLI and agents use the same admitted
   service operations and permissions. Meaningful navigation, selection, drafts,
   focus and view state have documented programmatic access. Revision checks
   protect newer human intent from stale writes. Authorized assistants must not
   be blocked by UI-only actions.
4. **Participate appropriately.** Normally respond to direct/addressed input;
   allow silence and avoid duplicate contributions in groups. Discussion is not
   acceptance of an assignment. Connections identify implementation and installed
   individual where available.
5. **Be honest about support and outcomes.** Negotiate callbacks/content types.
   Authentication failure preserves the saved session, without replacement or
   uncertain-effect replay. Receipt, execution, delivery, artifact verification
   and requester acceptance are distinct; reports identify the artifact and check.
6. **Minimize total setup effort.** Reuse configuration and automate credentials
   where allowed. Count decisions, clicks, switches, copying and recovery for
   administrators too. Preserve [self-contained installations](family.v1-candidate.md#self-contained-installation-baseline).
7. **Show useful activity.** Once committed to a response, show scoped present-tense
   tool activity, falling back to Thinking… for model wait; clear it on settlement.
   Use the richest supported indicator under [adapter activity](adapter-activity.v1-candidate.md).
8. **Let input continue work.** Steer through negotiated capability or queue for
   the next supported boundary. Preserve attribution, order, revisions, audience,
   completed results and unchanged approvals. Receipt is not incorporation; extra
   guidance is not Stop. [Input continuity](input-continuity.v1-candidate.md) owns
   the detailed promise.

## Protect attention, keep work visible

Routine coordination belongs in a discoverable conversation accessible to the
authorized human audience. People can inspect without prompting an agent, or
join active brainstorming. Access, notification/spoken delivery and authority
to act are separate.

Meaningful progress, results, changed commitments and decisions needing judgment
return to the original authorized destination. Reporting an accepted responsibility
is not optional ambient news. Retain important dissent and uncertainty. Reuse
existing participation/delivery policy, not another classifier or blanket quota.

Reuse an appropriate authorized conversation before creating one. Document platform
notification limits. Where no suitable room exists, an independently configured
Conversations service may supply a fallback; otherwise explain the limitation
without claiming hidden coordination or flooding the channel. Concrete operations
belong to the [huddle service](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/huddles.md).

## Verification expectations

- Compare UI/API effects with equivalent authority; reject stale view writes and
  keep late replies in their original destination.
- Revoke source access, then attempt cross-room retrieval. Reconnect after auth
  failure without creating a replacement session or repeating work.
- Compare direct requests, brainstorming and handoffs; retain artifact hashes,
  evaluation, delivery and requester decisions as distinct evidence.
- Follow a fresh setup through failure, recovery and first success, counting all
  user and administrator steps.
- Send follow-ups during model calls, tools and approval waits, with and without
  steering and across disconnect. Check order, effects, approval continuity,
  receipts and final destination.
- Observe quiet coordination, inspection, joining and unfollowing. Measure
  interruptions, useful contributions, missed decisions and comprehension effort;
  distinguish Stop and revoked access.
- Confirm a huddle link grants no access, inspection grants no work authority and
  peer posts cannot become human instructions. Test notification preferences,
  fallback and uncertain-delivery recovery: preserve source/time and evidence
  without duplicate reports or replay.

## Left open

Controls and optional callbacks may differ by host; document supported behavior.
This agreement creates no global identity directory, universal memory or automatic
acceptance of work on someone's behalf.
