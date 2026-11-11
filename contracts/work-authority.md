# Accepting and changing work

For experiences taking requests, agents accepting responsibilities, and services
that enforce actions. The promise: people can authorize, redirect and stop useful
work in conversation, with honest acceptance and limits that survive delegation
and recovery. This is a behavioral agreement, not a new permission API.

**Example:** A person asks a coordinator to prepare a briefing with two helpers.
The helpers accept within the person's scope. Reducing the allowance preserves
work already spent; a lost acknowledgment does not create another assignment.

## Commitments

1. **Make the effective policy understandable.** Each installation owns intake,
   defaults and ceilings. Expose acceptance/amendment/delegation availability,
   chat-only versus temporary capacity, applicable limits and usage, separate
   provider/tool constraints, and a useful reason/remedy when unavailable through
   its supported UI, programmatic controls and agent context. Tool names alone
   prove no authority. Waiting cannot turn chat-only policy into execution.

2. **Trust the source, not a claimed identity.** Authenticate actor kind, source,
   revision, original time and audience through the connected service. If policy
   permits any authenticated human participant, do not silently narrow it to an
   owner-only DM or allowlist. A quotation, bot assertion or peer copying a human
   request cannot create human authority. Narrower installation policies remain
   possible. Reply admission permits effects only under separately valid work
   authority; ambient quotas must not silently block eligible human intake.

3. **Accept once, durably.** Ordinary agent reasoning interprets the request; no
   obligatory extra model or keyword classifier is required. The resource owner
   validates admission, then commits assignment identity, goal, source, limits
   and deduplication together before reporting acceptance. Questions and peer
   suggestions are not assignments. Distinguish queued, running, delivered and
   verified results. Acknowledge briefly; clarify only consequential missing
   information. Retain supported pending requests rather than asking people to
   repeat them because setup lacked a transition. Later admission still requires
   current policy and a valid, unretracted source; uncertain admission is not
   retried blindly and old history is not silently promoted to work.

4. **Represent limits truthfully.** Each dimension is finite or explicitly has no
   configured bound. Unknown, omitted and unavailable never mean unlimited. A
   completion-bound goal still has scope, owner, identity, Stop and actual effect
   receipts; independent installation/provider/tool limits remain effective.
   Changing defaults applies to new work, never converts finite or revives
   expired, stopped or uncertain work. Do not use sentinel numbers, rolling
   deadlines or refreshed allowances to simulate unbounded authority.

5. **Amend the existing responsibility.** Bind a change to the assignment and
   expected revision, retaining source, old/new limits, consumed work, completed
   effects and unknowns. A cumulative cap is not a new unused balance. Reject or
   constrain requests honestly within policy and ceilings. Removing or extending
   limits requires authorized amendment; smaller already-exhausted limits close
   new effect admission while running operations settle under their cancellation
   contract. Concurrent amendments fail or reconcile visibly. Retries recover
   the same outcome, not another allowance.

6. **Honor Stop and recovery boundaries.** Stop precedes continuations and peer
   requests. Later ordinary messages and mode changes do not implicitly resume
   work. Source expiry, retraction, materially changed scope or ambiguous effects
   can require fresh confirmation. Preserve useful completed work and inspect
   uncertain effects before further related action; follow
   [work continuity](work-continuity.md).

7. **Delegate without inventing permission.** Explicit human authorization to
   choose a lead or recruit helpers avoids redundant human fan-out. Each recipient
   accepts independently under its policy, tools and limits. Handoffs identify
   original authorization, subtask, recipient, audience, bounds and further
   delegation rights. Verify the service-owned original or a supported authenticated
   cross-experience delivery; a peer assertion or copied JSON is insufficient.
   Aggregate allowances require accountable allocation, not multiplication.
   Separately authorized per-agent allowances remain distinct. Delivery,
   acceptance, incorporation and completion have separate evidence; private
   material retains its audience. Unsupported delegation is an honest limitation.

8. **Keep enforcement with its owner.** Instance may expose responsibilities;
   domain services enforce their resources and action leases. A work ledger or
   export/import records material, not transferred execution authority. Stronger
   behavior needs an explicit supported interface. Local adapters may implement
   intake without changing engines, adding an ACP method or requiring a fleet
   service. Optional [watch](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/watches.md)
   and [continuation](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/continuation.md)
   helpers require compatible bound discovery; watches retain finite TTL, identity,
   delivered evidence and accounting. Advice never substitutes for effect checks.

**Check:** Exercise a question versus an eligible request, an open human policy
versus a forged peer claim, duplicate/lost-ack/restart admission, chat-only versus
capacity, and amendment races. Reduce an allowance below consumption and issue
Stop during execution. Delegate under aggregate and separate budgets. Verify
actual effects, unchanged identity/counts/unknowns and truthful acceptance,
progress and return to the original conversation.

**Left open:** Domain task schemas, supported operations, scheduling and local
intake implementation. Releases and installation configuration establish actual
support; adopting this agreement does not grant execution authority. Keep bounded
private diagnostic identifiers and reasons, not task contents or credentials.
