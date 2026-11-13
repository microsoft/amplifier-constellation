# Accepting and changing work

For agents and services that accept requests or enforce actions. People can authorize,
redirect and stop work with limits that survive delegation and recovery. This defines
behavior, not a new permission API or mandatory task schema.

Assignment acceptance and amendment apply when taking ongoing responsibility.
An ordinary tool call can use existing authority and its declared operation contract
without creating another assignment; resource access and effect checks still apply.

**Example:** A coordinator delegates a briefing. Reducing its allowance preserves work
already spent; a lost acknowledgment creates neither another assignment nor allowance.

## Commitments

- **Expose effective authority.** Show supported intake, amendment and delegation,
  chat-only versus work capacity, limits and consumption, separate provider/tool
  constraints, and useful reasons/remedies. Waiting or a tool name creates no authority.
  Authenticate the source; quotations, copied records and peer assertions are not human
  grants. Honor configured policy, including open human participation without silently
  imposing owner-DM or allowlist gates. Reply admission does not grant effect authority;
  ambient quotas do not block eligible intake.
- **Accept once.** Reasoning may interpret requests without an extra classifier.
  Validate and durably bind assignment identity, goal, source, limits and deduplication
  before claiming acceptance. Questions and suggestions are not assignments. Clarify
  consequential gaps only. Retain supported pending requests, but admit them under
  current policy and valid unretracted sources; do not promote old history to new work.
- **Amend without renewal.** Limits are finite or explicitly unbounded; unknown,
  omitted or unavailable is not unlimited. Completion-bound work still has scope,
  owner, identity, Stop and effect evidence. Defaults affect new work, not expired,
  stopped or uncertain work. No sentinel caps, rolling deadlines or refreshed allowances
  may simulate unbounded authority. Amend the existing responsibility against its
  expected revision, retaining source, consumption, effects and unknowns. A cumulative
  cap is not a fresh balance. Reconcile conflicting changes; retries recover the same
  outcome. An exhausted lower cap closes new effects while started operations settle.
  Only authorized amendments may remove or extend limits, within policy and ceilings.
- **Honor Stop.** Stop precedes continuations and peers. Ordinary messages or mode
  changes do not resume work. Expiry, retraction, changed scope or ambiguous effects
  can require fresh confirmation. Revocation still applies after execution loads.
  Preserve progress and follow [continuity](work-continuity.md).
- **Delegate within the grant.** Permission to recruit avoids redundant human fan-out.
  Recipients accept independently; the sender remains responsible until acceptance.
  Handoffs identify verified original authorization, subtask, recipient, audience,
  bounds and further delegation rights. Allocate aggregate allowances without
  multiplying them; separate authorized allowances remain distinct.
- **Enforce at the resource.** Work records, exports and advice do not transfer
  execution authority. The resource owner enforces current grants. Unsupported intake
  or delegation is an honest limitation. Domain mechanisms need no new engine, ACP
  method or fleet service; optional helpers follow their [profile](profiles/acp-hosting.md).

**Check:** Distinguish questions, requests and forged grants. Lose an acknowledgment,
restart, amend concurrently, reduce limits below consumption, Stop and delegate.
Inspect effects, identity, consumed limits and unknowns, not just replies.

**Left open:** Intake, task schemas, scheduling and domain operations. No universal
permission service or routine confirmation gate is required.
