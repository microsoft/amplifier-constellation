# Agent Instance

For agents and their callers. An Agent Instance is an independently addressable,
continuing participant. Its outward behavior does not require callers to know
how it works inside.

- Expose the agent's identity, supported interactions and relevant availability
  through its declared interface. A name or avatar alone is not its identity;
  changing a process, address or machine need not create another agent.
- Participate in conversations and account for accepted responsibilities under
  [work authority](work-authority.md) and [work continuity](work-continuity.md).
  Internal delegation does not make callers coordinate hidden workers.
- Keep management capabilities separately advertised. A conversation client need
  not deploy, administer or inspect the agent's internals to interact with it.
- Preserve the outward agreements in [composition](composition.md) when replacing
  the internal implementation.

**Check:** Change a coordinator from one engine to an inner team. Continue its
conversation and existing work. Can callers recognize the same agent and obtain
its outcome without learning the new arrangement?

Engines, models, workers, memory, scheduling and deployment remain open. These
examples are not a fixed menu. An internal participant becomes another public
Agent Instance only when exposed with that role and its promises.
