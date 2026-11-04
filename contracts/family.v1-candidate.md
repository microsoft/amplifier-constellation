# Family contract — v1 candidate

Draft for review · Updated November 3, 2026. This governs how independently owned components compose.

## Who builds against this

The umbrella repository maintains the component map and shared expectations.
Runtime, Instance, experiences, connectors, Persona Workshop, and ACP Lab own
their implementations and detailed public interfaces.

## What it is

The registry describes components and their publication status. It is documentation,
not an executable installation catalog or a grant of access to an agent.

```toml
[[component]]
id = "runtime"
repository = "https://github.com/microsoft/amplifier-constellation-runtime"
scope = "constellation"
publication = "pending"
purpose = "Native Rust ACP agent engine"
```

## Promises

1. **One owner per interface.** Detailed schemas and implementation tests live in
   the component that serves the interface. The umbrella links to them rather
   than keeping competing copies.
2. **Independent components remain usable.** Installing one agent or connector
   does not require the fleet dashboard or a shared Constellation setup service.
   Independence includes setup and ongoing management, not only message delivery.
   Stopping an optional management service does not stop independent services or
   delete their state.
3. **Each individual owns its state.** Different agents have separate identities,
   homes, credentials, and lifecycle. Components do not depend on another
   individual's private files or database schema to communicate.
4. **Standard ACP is sufficient for chat.** A compatible ACP endpoint can join an
   experience without the Constellation runtime or management API. Optional
   capabilities are negotiated and unavailable features are explained.
5. **Extensions declare their requirements.** Management and context discovery
   name their version and required authorization. Custom remote transports are
   identified as such; an ACP-shaped URL is not a claim of universal compatibility.
6. **Replacement preserves declared meaning.** A different runtime or connector
   may satisfy the same interface. Identity retention and session portability
   must be explicit; incompatible saved state is not silently reused or discarded.
7. **Publication state is factual.** A planned repository is marked pending.
   Reachability, a release, and a tested integration are separate facts. The
   registry does not imply that every combination has been tested.
8. **Boundary changes are reviewable.** Changes identify the affected producers
   and consumers, migration behavior, and a check that observes both sides.
   Independent releases retain explicit compatibility information.

## Self-contained installation baseline

The current architecture decision is to keep each agent and connector installation
self-contained. Multiple people can own agents on different machines without a
mandatory shared provisioning, enrollment, credential, or identity-management
service. This is a design requirement; it does not claim that every component has
already automated its complete lifecycle.

- **Own the full lifecycle.** An installation provides a standalone path for
  setup, authorization, credential renewal, connection and image updates,
  recovery, and disconnection. Platform-specific operations belong to the
  connector; they do not become responsibilities of the ACP agent runtime.
- **Keep helpers local and replaceable.** An installation may bundle or invoke
  platform tooling to automate setup. Its authorized operator can complete the
  same lifecycle without registering with a shared Constellation service or
  receiving another person's administrator credentials. External platform
  authorization, network access, and administrator approval still apply.
- **Allow independent iteration.** Different installations may use different
  versions and implementations without requiring other owners to upgrade in
  coordination. Standard ACP and explicitly negotiated optional interfaces still
  apply; independence does not mean abandoning declared protocol compatibility.
- **Keep fleet management optional.** A higher-order experience may invoke an
  installation's exposed controls, but must not become the sole owner of state,
  credentials, or operations needed for its standalone lifecycle. A shared
  provisioning service is not part of the baseline. Introducing one as an
  optional convenience requires an explicit proposal that preserves this path.
- **Measure the whole setup journey.** Count operator and administrator decisions,
  clicks, app switches, copy/paste, and recovery steps. Moving work to an
  administrator or another service does not count as eliminating it.

For example, two owners can connect agents from separate machines to the same
conversation platform. Each installation manages its own connection under that
platform's permissions. Updating one connector must not require a shared
Constellation service update or an update to the other owner's installation.

## Verification expectations

These are acceptance checks to maintain, not a claim that a family conformance
runner already exists:

- Run ordinary chat with an independent ACP peer lacking every optional extension.
- Operate two agents with separate homes and versions; stop the fleet UI and
  confirm both still accept authorized direct connections.
- From a fresh installation, complete setup and authorization with optional
  Constellation services absent. Exercise credential renewal, image updates,
  reconnection, and disconnection through the standalone controls.
- Exercise two owners' installations on different machines and connector
  versions. Update one without updating the other; verify that one owner's
  connection credentials do not authorize management of the other's agent.
- Replace a configured backend and verify that unsupported session operations
  fail explicitly rather than inventing successful persistence.
- Validate the registry and distinguish pending, published, and tested claims.
- Trace each cross-component call to a declared interface instead of a private
  data path.

## Outside this draft

This contract does not define a universal agent state format, discovery network,
automatic trust federation, or new ACP transport. Those require separate proposals
when concrete implementations need to interoperate.

## Changelog

| Date | Change | Basis |
| --- | --- | --- |
| 2026-11-03 | Require a self-contained setup and management lifecycle; shared provisioning remains outside the baseline. | Independent iteration across owners, machines, and implementations without a mandatory shared service or coordinated upgrades. |
| 2026-11-02 | Initial candidate. | Independent installation, ACP compatibility, and optional fleet-management requirements. |

## Installation updates

The [installation updates contract](installation-updates.v1-candidate.md) adds
optional local lifecycle controls. Each installation owns its helper and policy;
Observatory is only a client. Agent-initiated updates use one instance-wide enable
switch, without per-requester or owner-DM authorization when enabled.
