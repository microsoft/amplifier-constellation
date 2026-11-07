# Installation updates — candidate contract

Updates belong to each installation. A shared provisioning, identity, credential,
or fleet service is not required. An optional machine or fleet client discovers
and invokes the installation's management capability. ACP remains unchanged.

An installation owns a host helper, enrolled component providers, durable plans,
version receipts, schedule, and recovery. Runtime, management host and connectors
remain independent components. An installation may enroll a connector it owns;
this does not move connector setup, platform authorization or identity into the
runtime. Other software may expose the same operations through an owner-enrolled
update provider. Enrollment never comes from model-generated paths or commands.

The `constellation.installation-updates/1` capability provides:

- Status with installed/available versions, source provenance, helper health,
  schedule, pending request, plan identity and outcome.
- Check, plan, apply, rollback, and update (check then apply) operations.
- Idempotent request identities and durable progress across service restarts.
- A single instance-wide **Allow agent-initiated updates** setting. When enabled,
  any participant may ask the agent to update from any connected conversation.
  Caller identity, owner-DM routing and participant allowlists do not add a second
  authorization gate. This choice does not grant broader host authority.
- Hourly checks by default, with manual, 15-minute, six-hour and daily alternatives.
  Checks and installation are separate; on-request installation is the default.
- Optional automatic installation when idle, optionally in a scheduled UTC window.
- Stable-release and latest-passing-development channels, with explicit source
  and artifact provenance and no silent downgrade.

Dashboard, CLI and agent tools invoke the same operations. Browser route, draft,
focus and action state remain programmable through the existing view contract.
Receipts distinguish accepted, waiting, applied, verified, rolled back and failed;
queuing an update is not evidence that it completed.

For containers, the updater runs outside the container and accepts only enrolled
operations. It does not expose a Docker socket, arbitrary host shell or writable
host helper state to the agent. Existing work and pending replies drain before
cutover, with admission closed against a check/start race. Configuration, identity,
credentials and conversation stores survive. Verification includes renewed
connections; failure restores the prior executable without overwriting messages
received since the update began. Recovery survives both agent and helper restarts.

A component that does not implement this optional management contract remains a
normal ACP agent. A component provider may wrap it without modifying ACP or
requiring Constellation-specific conversation behavior.

## Complete component coverage

When enrolled, the same update request and schedule cover the Instance, its native
or external ACP engine, connectors and their dependencies, separate local helper
executables, and its container image. Enrollment and status must identify any
component not managed by this installation; an updated host is not a claim that
all attached software is current. External engines follow their own upstream
release artifacts or immutable source revisions, preserving engine family,
settings, provider accounts and sessions. Wrapping update operations does not
justify a fork, custom ACP transport, or replacement of an engine's dependency
graph. Staging freezes resolved dependencies before cutover.

Automatic installation is an owner-selected policy. When selected, scheduled,
agent-requested and dashboard updates use the same plan, idle checks, verification
and recovery. They do not require recurring operator approval. A busy installation
waits; it does not cancel a turn merely to install an update. A failed check or
recovery must remain visible rather than being reported as current.

## Connected-experience lifecycle

An owner may enroll installation-local lifecycle participants. Each declares
which component IDs affect it and fixed `prepare`, `verify`, and `restore`
commands. These are host management commands, never model-supplied commands or
ACP extensions. Enrollment is frozen into the plan; changes invalidate application.

Each application or explicit rollback has a fresh durable transaction identity.
Participants receive an immutable context containing that identity, plan identity,
operation, changed components and restart scope. They persist original service,
mask, admission and continuity state before mutation. Preparation closes local
admission atomically while existing work settles, before the Instance closes its
own admission. A merely idle status sample is insufficient.

After all affected participants are ready, the helper applies the frozen component
plan. Verification checks component identity and connected-experience continuity.
Restoration preserves originally stopped/masked services, original task authority,
usage, sessions and unknown outcomes; it never renews tasks, replays uncertain
inputs, or restores old state over new observations. A failed update restores the
prior software, then verifies and restores the experience.

The helper persists successful verification before restoring any participant.
Once restoration starts, interrupted recovery finishes restoration only: a resumed
listener may already have done new work, so rolling its executable back then is
not a safe substitute. Lost acknowledgments must be handled idempotently. Recovery
that cannot establish continuity remains explicit and blocks further updates.
