# Family contract — v1 candidate

Draft for review · November 2, 2026. This governs how independently owned components compose.

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
   does not require the fleet dashboard. Stopping the dashboard does not stop
   independent services or delete their state.
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

## Verification expectations

These are acceptance checks to maintain, not a claim that a family conformance
runner already exists:

- Run ordinary chat with an independent ACP peer lacking every optional extension.
- Operate two agents with separate homes and versions; stop the fleet UI and
  confirm both still accept authorized direct connections.
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
| 2026-11-02 | Initial candidate. | Independent installation, ACP compatibility, and optional fleet-management requirements. |
