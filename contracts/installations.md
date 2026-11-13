# Independent installations and connections

For owners and clients of Constellation's locally managed installations,
connectors and enrolled update helpers. This is an implementation agreement:
people can set up, update, connect and recover these installations without losing
identity or work, depending on a central fleet service, or managing every step
by hand. A different Agent Instance architecture need not adopt this helper,
container or selected-engine arrangement; it still keeps the shared
[composition](composition.md) and [work continuity](work-continuity.md) promises.

The native single-computer target below applies to the locally hosted
Constellation family. External services and independently supplied engines keep
their stated requirements. Optional management features must be advertised;
ordinary ACP participation does not require them.

**Example:** Disconnect an agent from one world while leaving its other conversation
available. Update supported software, then reconnect to the same saved session.
An expired assignment stays expired; a missing acknowledgment does not repeat an
uncertain action.

## Commitments

1. **Keep the installation in charge.** Each agent and connector owns setup,
   authorization, credential renewal, state and recovery. Platform setup belongs
   to the connector, not the runtime. Optional fleet clients invoke exposed
   controls. Installation-owned helpers accept only enrolled operations and fixed
   commands, never model-generated paths or shell. Container helpers run outside
   the agent container without exposing a host shell, Docker socket or writable
   helper state to the agent. ACP peers remain usable without this management API.

2. **Make policy and coverage explicit.** Status identifies managed and unmanaged
   components, installed/available versions, provenance, helper health, schedule,
   plan and outcome. Enrolled coverage can include Instance, its selected engine,
   connectors and dependencies, separate helpers and container images. Resolve
   immutable release/source and dependency versions before cutover; do not silently
   downgrade, replace an engine family or alter its accounts, settings or sessions.
   Updating a wrapper does not justify a fork or substituted dependency graph.

3. **Use one authorized control path.** Dashboard, CLI and agent tools use the same
   operations. Agent-initiated updates have one installation-wide switch: when
   enabled, any conversation participant may request them, without another owner-DM
   or allowlist gate. This grants no broader host authority. Checks default to
   hourly; installation defaults to on-request. Owners choose channels, check
   cadence and optional idle installation under the [supported settings](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/updates.md).
   Checking and installing remain separate operations. Automatic policy uses
   the same checks and recovery without recurring approval; busy work waits.

4. **Freeze and recover the actual operation.** Record request identity, revision,
   plan and enrolled participants before effects. Changed reuse rejects. Each
   apply or explicit rollback has a fresh transaction with immutable scope.
   Participants persist original service, mask, admission and continuity state;
   atomically close local admission while work settles, before host drain. An
   idle sample alone cannot prevent new work racing a cutover. Connection changes
   and updates serialize through the same helper/domain boundaries.

5. **Preserve state through verified restoration.** Verify software identity,
   original-session compatibility and connected-experience continuity before
   restoring listeners. Keep identity, configuration, credentials, messages,
   task authority, consumed allowances and unknown outcomes. Preserve services
   already stopped or masked; never renew expired work or overwrite new observations.
   On failure, restore prior software, verify it, then restore the experience.
   Persist verification before restoration starts. After restoration begins,
   interrupted recovery finishes restoration only: resumed work makes a new
   rollback unsafe. Lost acknowledgments reconcile idempotently. Uncertain
   continuity stays visible and blocks further updates.

6. **Save connections independently.** Every connection has its own identity,
   revision and desired versus observed state. Multiple connections must not
   collide with other registrations, credentials or updates. Add validates supported
   setup inputs and saves a descriptor; it grants no work, creates no agent session
   and enrolls no unrelated software. Advertise Add only with usable domain setup;
   existing-only catalogs remain useful. Clients cannot invent approved routes,
   credentials or unused installations.

7. **Disconnect safely; reconnect deliberately.** Disconnect preserves saved work
   and credentials. Keep context available while active work settles; acknowledge
   domain quiescence and context closure before stopping its endpoint. Otherwise
   an unreachable provider can break unrelated conversations. On Connect, a
   reachable endpoint may become context-ready under closed effect admission for
   original-session verification. Persist and acknowledge readiness before advancing;
   it is not terminal success. Metadata, enablement and effective availability
   remain distinct. Offline connections defer updates needing unavailable checks,
   without temporary joins, fabricated verification or disabling unrelated schedules.

8. **Report and reconcile honestly.** Acceptance, process health, readiness and
   successful connection/update are different outcomes. Status may advance an
   already-journaled operation; unknown effects never justify new dispatch or
   clearing a request as failed. A settled failure may close it. Preserve unrelated
   edits through supported conditional mutations, not stale whole-registry writes;
   an old setup grant cannot re-enable a subsequently disabled provider.

## Native single-computer setup

The locally hosted Constellation family must work together on one computer:
Windows 11 on x64 or ARM64, macOS on Intel or Apple silicon, and Linux on x64 or
ARM64. Ordinary setup must not require WSL, a separate Linux machine, developer
tools or a user-installed language runtime. Owners package or privately install
their runtime dependencies and provide native setup, startup, updates and recovery.
Additional machines, containers and fleet clients remain optional.

External services and independently supplied engines keep their account,
licensing, network and platform requirements. Show these accurately before setup;
do not silently substitute an engine, account or compatibility layer. A native
support claim requires the actual package and combined journey on the named OS
and architecture, including ordinary user launch, connection, restart and saved
state recovery. A successful cross-build alone is not that evidence. Components
publish their verified OS floors and exceptions; this agreement states the target,
not that every component has already reached it.

Concrete capabilities, commands, APIs and lifecycle ordering belong to Instance's
[updates](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/updates.md)
and [saved connections](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/service-connections.md),
with domain setup and verification owned by each connector.

**Check:** Exercise two connections and an unrelated catalog through setup,
active-work disconnection, update, rollback and reconnect. Verify original sessions,
legitimate offline expiry and unchanged authority. Inject crashes, lost reports,
concurrent edits and failures after restoration starts; count effects and inspect
new observations. Unsupported Add has no dead control. Failed checks remain visible.
Measure administrator effort alongside user effort.

**Left open:** Panel layout, packaging and platform-specific setup mechanics. No
central provisioning service, arbitrary host-command API or new ACP behavior is
required.
