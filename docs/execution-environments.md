# Shared execution environments

Design proposal · November 4, 2026. Implementation remains paused. Existing
per-agent containers are an interim containment measure, not this new product.

## Decisions already made

- A project can share tools, repository objects and build caches among agents.
- Each agent keeps its own identity, credentials, conversation state and grants.
- Separate working copies are the default. Shared writable folders are explicit.
- Agents retain Internet access while access to the host filesystem is restricted.
- One environment may host several agents; another may serve only one. Placement
  is independent of an agent's social role or reporting relationship.

Hosting an agent service and executing its assignments are separate choices.
Containing the whole installation does not require every project to share that
container or equate an environment with Docker.

## Proposed model

An **environment** is an owned execution boundary with a versioned definition:
base software, resource limits, allowed network destinations, declared mounts,
shared caches and recovery policy. A **workspace** is a checkout or explicitly
shared folder inside it. An **attachment** lets one installed agent execute in a
workspace under a particular permission set and lease.

The environment owns execution resources, not agent identity or conversation
history. The agent owns its responsibilities, not the host's administration.
An agent can use different environments for different tasks. A shared social
space does not automatically grant access to any shared filesystem.

## Default behavior proposed for review

| Operation | Proposed behavior |
| --- | --- |
| Create | Owner selects a project and tools; an editable template supplies limits. |
| Attach an agent | Give it a fresh working copy and a named, revocable execution lease. |
| Share files | Publish an artifact or opt into a particular shared directory. |
| Use credentials | Provide task-scoped access for that agent; never copy all host credentials into the environment. |
| Leave | Revoke new execution; drain or explicitly cancel active work; retain an inspectable receipt. |
| Update | Stage the new definition, test it, then move idle execution; do not mutate an active toolchain underneath work. |
| Stop | Prevent new commands, stop processes, and report uncertain external effects. |
| Recover | Inspect durable execution and external receipts; retry only when the effect is known safe. |
| Delete | Detach agents, show retained artifacts and uncommitted work, then require explicit destructive confirmation. |

Sharing a tool cache needs its own trust choice. One agent's writable executable
cache can affect another agent. Prefer immutable tool layers and partitioned
write caches; share untrusted build outputs only deliberately. Filesystem layout
alone does not establish isolation between processes in one container.

## Authority and network

An attachment must bind the agent, project and originating conversation audience.
A public-room request cannot borrow permissions from the same agent’s private
project. Repository instructions and persona files cannot change these grants.
If a selected execution environment is unavailable, fail visibly rather than
silently falling back to commands on the host.


The owner administration service runs outside the agent's execution boundary.
Its APIs accept declared operations, not arbitrary host shell commands. An agent
may request a workspace or an allowed tool installation according to owner policy.
It cannot convert that request into host root access, mount the owner's home, or
obtain a container-manager socket.

Internet egress and access to local infrastructure are separate controls. Allow
ordinary Internet work by policy; private-network services need declared routes.
Existing Minecraft connections must be represented explicitly and tested at the
actual ACP/MCP boundary. No environment operation resets a world, renews a task's
deadline, or expands its execution budget.

Credentials should be issued per agent and task where the external platform
supports it. Where it does not, document the limitation. A shared account secret
is not separate agent authority merely because it is copied into separate files.
Prefer a narrowly scoped credential broker or process injection that avoids
persisting credentials in shared checkouts; this is a proposed local mechanism,
not a new mandatory shared Constellation identity service.

## Lifecycle and failure contract

Use explicit states: preparing, ready, draining, stopped, failed and deleting.
Execution leases carry agent identity, workspace identity, capabilities, expiry
and environment revision. Expiry stops new execution but does not imply an
external side effect was undone. Preserve request IDs and receipts across retries.

A host crash leaves running commands uncertain until process and effect evidence
is reconciled. Recovery must distinguish an interrupted process from an unknown
remote write. Working copies and delivered artifacts have separate retention
policies. Restoring an environment image must not roll back newer work records or
conversation messages.

Concurrent edits in a deliberately shared folder need conflict visibility and
explicit coordination. A lock is appropriate for an exclusive operation, not a
promise that agents will never disagree. Repository review remains the normal
integration path for separate working copies.

## Proposed interface boundary

Expose capabilities and versioned operations for create/inspect/update/drain/stop,
workspace create/snapshot/export, attach/revoke, execution submit/cancel/status,
events and recovery receipts. Keep a CLI, management API and optional UI on the
same control path. All visible navigation and selection state must also be
programmatically controllable.

ACP remains the reasoning-agent boundary. An environment execution capability is
an explicitly authorized tool or host service; do not pretend environment
provisioning is a standard ACP feature. External ACP agents can use it through
supported tool interfaces without adopting the Constellation runtime.

Each installed environment service must work independently. Fleet management
can discover and operate it as an optional client. Define implementation-neutral
capabilities before choosing a runtime technology or repository name.

## Questions to settle before building

1. Are agents sharing one process-security boundary trusted peers, or must the
   environment also isolate them from each other? The latter needs per-agent
   processes/containers or a stronger boundary within the shared project.
2. Should one agent attach to several environments concurrently, with task-level
   selection, or should the first version allow one active attachment at a time?
   The proposed default is task-level selection with a visible current target.
3. Who can approve shared writable folders and changes to network routes? The
   proposed default is the environment owner, with explicit delegated administration.
4. Which retention policy is acceptable for failed workspaces and uncommitted
   changes? The proposed default is retain until the owner reviews or expires them.
5. Which portable execution implementation meets the Linux, Mac and Windows
   requirements? Evaluate containers and VMs against the same containment and
   recovery contract; do not select one from its API shape alone.

Validate these choices through a design review. Do not interpret this proposal as
permission to implement the service, provision new environments or migrate agents.

Owner readiness should verify a checkout, dependency preparation, a representative
command and artifact access inside the actual target. A connected GitHub account
alone does not demonstrate that the agent can build or publish a project.
