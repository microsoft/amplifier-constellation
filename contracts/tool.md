# Tool

For capability providers and callers. A Tool offers a defined capability without
requiring an agent to absorb its machinery or treat it as a social participant.

- Define accepted inputs, returned outcomes, operating limits and supported
  interfaces under [composition](composition.md).
- Associate responses with their request. When work outlives its initial
  response, provide a stable operation reference and explain how further outcomes
  are obtained. Advertise supported progress, questions, continuation,
  cancellation and failure behavior without prescribing callbacks or polling.
- The provider is responsible for its accepted operation; the caller remains
  responsible for its place in the larger assignment. Apply
  [work authority](work-authority.md) and [work continuity](work-continuity.md).
- Sharing follows [composition](composition.md). One tool may serve many agents;
  a private tool is the same role with a narrower audience.

**Check:** Two agents commission investigations. Interrupt one connection, then
retrieve outcomes and answer a clarification. Do operations remain distinguishable
and avoid blind resubmission or questions going to the wrong caller?

Intelligence, duration, hosting and internal agents remain open. A simple tool
needs no job service, persona or human-conversation membership. Its product may
also expose an experience or Agent Instance using the same underlying work.
