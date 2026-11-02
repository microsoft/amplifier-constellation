# Experience contract — v1 candidate

Draft for review · November 2, 2026. This governs how an experience connects people, agents, and actions.

## Who builds against this

Conversation experiences, platform connectors, agent services, and management
interfaces share these expectations. A platform-specific adapter owns the
translation to its platform's presentation, routing, and access rules.

## What it is

An experience submits a scoped interaction and shows the result at its source.
The diagram is conceptual; exact payloads belong to the producing component.

```text
actor + conversation + audience + optional thread + destination
  -> authorized ACP interaction or management operation
  -> recorded outcome
  -> delivery in the intended place
```

## Promises

1. **Keep the originating scope.** Actors, conversations, threads, and destinations
   remain identifiable through execution and delivery. A late reply cannot move
   to a newly selected conversation or a different audience.
2. **Context does not grant access.** An excerpt or persona instruction cannot
   authorize a tool or expose another conversation. Current source grants and
   destination membership govern retrieval and sharing.
3. **Use the same admitted action.** Clicks, keyboard, voice, CLI, and agents reach
   the same service operation and permission checks. UI-only actions are defects
   when they prevent an authorized assistant from operating the app for a person.
4. **Expose meaningful view state.** Navigation, selection, drafts, focus, and
   other meaningful visible state have a documented programmatic path. Revision
   checks prevent a stale controller from overwriting a person's newer intent.
5. **Preserve the individual across places.** Connections identify the agent
   implementation and the installed individual where available. Recognizing the
   same agent does not imply a single transcript or unrestricted shared memory.
6. **Participation respects the interaction.** Direct or explicitly addressed
   messages normally receive a response. Group participation supports silence,
   avoids duplicate contributions, and distinguishes discussion from an accepted
   assignment. These are experience behaviors, not invented ACP capabilities.
7. **Show capabilities and failures honestly.** Negotiate callbacks and content
   types before using them. Preserve a saved session when authentication fails;
   do not silently create a replacement or replay uncertain side effects.
8. **Separate outcomes from claims.** Receipt, tool execution, delivery, artifact
   verification, and requester acceptance are different observations. Reports
   name the exact artifact and check rather than turning an agent's claim into
   proof that the user's larger task is complete.

## Verification expectations

These are acceptance checks to maintain, not a claim of complete coverage:

- Exercise UI and API paths with equivalent authority and compare their effects.
- Attempt a stale view update and a late response after switching conversations.
- Revoke a source grant and confirm it prevents subsequent cross-room retrieval.
- Reconnect after an authentication failure without recreating or repeating work.
- Compare a direct message with a group brainstorming and handoff scenario.
- Preserve the artifact hash, evaluator result, delivery receipt, and requester
  decision as distinct records.

## Outside this draft

The contract does not require identical UI controls or callback capabilities in
every host. Each host must document what it supports. It does not define a global
human identity directory, a universal memory system, or automatic acceptance of
work on a person's behalf.

## Changelog

| Date | Change | Basis |
| --- | --- | --- |
| 2026-11-02 | Initial candidate. | Same-agent continuity, natural participation, source-owned conversations, and programmatically operable UI requirements. |
