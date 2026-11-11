# Saved service connections — v1

For the installation registry, host helper, domain connector and clients that
compose their controls. The promise: a saved connection can be added, inspected,
connected or temporarily disconnected without losing its identity or work.
The [Instance interface](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/service-connections.md)
owns the concrete registration, command and API shapes. They are not duplicated
here or made part of ACP.

## Ownership

The domain installation owns setup, credentials, sessions, tasks, service
lifecycle and recovery. Instance exposes an optional saved registry and queues
supported operations. Its installation-owned helper serializes them with enrolled
updates. A browser, agent or other client requests a declared action; it cannot
supply host commands, executable paths or a shell. This is local composition,
not a central provisioning service.

Each connection has an independent identity, revision and observed state. Desired
state and actual state remain distinct. Multiple connections to the same platform
must not overwrite one another or collide with unrelated updater components,
participants, catalogs or credentials.

## Shared promises

1. **Save without implying connection.** Add validates the domain's supported
   setup inputs and returns one saved descriptor. It does not grant work, start
   an agent session or enroll unrelated updates. Expose Add only when a usable
   setup capability exists; existing-only catalogs remain useful without it.
   Domain setup owns platform authorization and route preparation. Clients do
   not invent approved destinations, unused installations or credentials.
2. **Use one durable operation.** Record the exact request identity, revision and
   intended action before effects. Changed reuse rejects. After a crash or lost
   acknowledgment, inspect that operation's status; do not redispatch it under a
   new identity. Status may progress already-recorded phases. Unknown is not a
   settled failure and cannot be cleared merely to try again.
3. **Preserve accepted work.** Disconnect retains configuration, credentials,
   individual identity, saved sessions, tasks, consumed allowances and unknown
   effects. The domain settles active work before stopping the relevant service.
   Connect preserves the original binding where one exists and grants no new
   authority. Expired or stopped work is not silently revived.
4. **Keep context reachable or explicitly unavailable.** A stopped domain must
   not leave an apparently active context provider that breaks unrelated
   conversations. Provider enablement and the connection's effective context
   availability are separate, preserving metadata and credentials for reconnect.
   Already delivered context cannot be retracted; subsequent reads obey current
   access and availability.
5. **Respect the drain and readiness handoffs.** On Disconnect, existing context
   remains usable while active work settles. The domain reports quiescence; the
   host acknowledges closing context before the domain stops its endpoint. On
   Connect, a reachable endpoint may become context-ready under closed effect
   admission so the original session can be verified. Readiness is not terminal
   connection success. Persist and acknowledge these reports before advancing
   the domain operation, including after a lost reply.
6. **Serialize with updates.** Connection operations and enrolled updates use the
   existing helper and domain lifecycle boundaries. A queued request is not an
   executing request. An offline connection defers an update requiring unavailable
   session verification; it does not join temporarily, pretend the check happened
   or disable unrelated scheduling.
7. **Preserve unrelated intent.** Append or update only the intended descriptor
   and configuration. If setup needs a conditional provider change, use the
   service's supported atomic operation; a stale read-and-replace of the whole
   registry cannot protect concurrent edits. A prior setup permission must not
   re-enable a provider deliberately disabled later.
8. **Report what settled.** Acceptance, process health, context readiness and
   successful connection are different observations. A terminal report binds to
   the original request and actual state. Known settled failure can close a
   failed request; ambiguous effects remain visible for reconciliation.

## Checks that matter

Exercise two connections and an unrelated catalog entry through Add, Connect and
Disconnect. Verify exact original session compatibility, authority and saved
state, including legitimate task expiry while offline. Test an active turn during
Disconnect, both context handoffs, concurrent registry changes, lost acknowledgments
and restart. Count dispatches and actual effects, not just visible status.

An unsupported Add path must expose no dead control. A disconnected provider
must not break an unrelated conversation. All clients use the same authenticated
operations; no particular panel layout is part of this agreement. The actual
domain's fixture and integration checks establish support for its lifecycle.
