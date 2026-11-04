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
