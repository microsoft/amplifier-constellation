# Amplifier Constellation

This defines the shared services and behavior we want. Product
experiences remain open to design; existing development and administration
screens do not establish the intended UX.

## Persistent agents, useful wherever people work

People can now direct more work than they can personally follow. The opportunity
is larger than getting another answer from a model: a small group can pursue an
ambition that previously needed a much larger organization. The difficulty is
keeping that work coherent, knowing what deserves confidence, and spending human
attention where judgment, expertise and taste make a difference.

Constellation provides reusable services for persistent agents to participate in
that work. An individual can act as a personal assistant, coordinator or
specialist across several experiences. It retains its identity and accepted
responsibilities while each conversation keeps its own audience and access
rules. People remain responsible for direction and delegated authority. Agents
carry preparation, execution, coordination, verification and follow-through
within their demonstrated capabilities.

## A common foundation for different experiences

Companion, Slack, Teams, Minecraft, Amplifier Unified and future web or native products
should be able to draw on these capabilities without rebuilding an agent system
for each surface. A person may talk, type, show something or work alongside an
agent. Each experience can design the interaction that fits its setting.

Products should also reuse shared Amplifier platform capabilities for organizing
work, handling resources and results, and presenting information. Companion and
Unified can draw on the same foundations while offering different experiences.
Browser and native client foundations make those experiences available on the
devices people use. They need not duplicate an agent system or a platform for
each product and device.

Underneath, the services should preserve the same essential meanings. A new
conversation need not create a new individual. Relevant context can be retrieved
with its source and permission intact. An accepted task can continue without
making the person relay every exchange. A clarification preserves useful work;
Stop remains explicit. A lost acknowledgment leaves an honest unknown outcome,
not an excuse to repeat an action.

Routine agent coordination belongs in an observable place away from the human's
main conversation. People can inspect or join when useful. Meaningful progress,
results, changed commitments and decisions return to the original conversation.
Multiple agents can contribute during active joint work; they need not narrate
routine exchanges. Quietness alone is not success: the work must remain
understandable, accountable and effective.

## Independently owned parts that work together

The foundation separates installed identity and management, reasoning and tool
execution, conversation and context services, and connections to external
systems. Standard Agent Client Protocol (ACP) connects clients and agents;
explicit tool and service interfaces connect the capabilities around them.
Enhancements are negotiated rather than assumed from a product name.

Each installation owns its setup, credentials, state, updates and recovery. It
can use a different engine, machine or release without requiring a central
Constellation service. Coordination never implies unrestricted access to another
agent's history or authority. A model, an engine session, an individual and an
assignment remain distinct.

These boundaries also let the people building Constellation lead substantial
areas in parallel. Early prototypes do not bound the ambition or the depth of
future investment. Ownership boundaries can evolve as people reshape the system
together. Owners can deepen a capability and make local choices while
others rely on its agreed interfaces. They draw on one another's expertise when
it can improve the outcome and realign when a shared promise changes. We use
[Converge](https://github.com/microsoft/amplifier-converge) to carry that direction
into agent-assisted work without requiring everyone to follow all the details.

## How we judge the result

We should be able to compose a useful journey across independently owned parts,
redirect it during work, recover from interruption, and deliver a result whose
claims can be checked. Evaluate the human effort to set it up, understand it,
correct it and trust it—not only the amount of work agents produce.

The services should make new experiences easier to build while leaving their UX
open. Current implementation is something to inspect against this intent when
choosing work, not a second specification. Simplify or replace mechanisms when
they consume more attention than they return.

The [family agreement](../contracts/family.v1-candidate.md) defines composition
and ownership. The [interaction agreement](../contracts/experience.v1-candidate.md)
defines what consumers can rely on. The [visual guide](guide/README.md) explains
the parts and connections; [working together](working-together.md) applies the
method to contributions.
