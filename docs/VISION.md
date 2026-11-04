# Amplifier Constellation — Vision

Draft for review · November 4, 2026.

This describes the intended experience. Shipped behavior and verification belong
in [implementation status](STATUS.md) and component release records. Changes to
this direction should be explicit before implementation changes the agreements
other components depend on.

## What Constellation is

Constellation lets people work with persistent agents as participants in their
everyday tools. A personal assistant can help across projects; a team coordinator
can organize shared work; project leads, engineers, and reviewers can own narrower
responsibilities. People can speak to any of them directly or work through a lead.
The work stays connected without making a person relay every conversation.

## Principles

### 1. An agent is an individual

A template supplies reusable behavior and capabilities. An individual carries an
identity, learned context, and commitments. An assignment gives that individual a
role in a setting. Two agents from the same template can own work independently;
one individual can hold several assignments. Changing models need not create a
different individual. The installed runtime family is chosen when creating an
installation. Updates within that family are supported; trying a different family
means creating a separate installation, with no automatic session migration.
Separate instances are also the unit for experimenting with different host policies.

### 2. Conversations are places to participate

Slack, Teams, and browser experiences own their conversations. An agent can join
several of them, and people should recognize the same individual in each place.
A conversation, a model working session, and an agent's lifetime are separate.
Continuity should not require placing all conversation history in one prompt.

### 3. Coordination preserves boundaries

A person can have separate work and personal agents plus an assistant that
coordinates both. Coordination does not imply unrestricted access or permission
to pass information between them. Source membership, current grants, and the
destination audience constrain recall, delegation, and replies.

### 4. Participation fits the room

In a direct conversation, a person normally expects a response. In a group,
agents should contribute when they can help, recognize an actual handoff, and
leave room for people and other agents. They should not repeat each other or turn
brainstorming into work assignments. Quiet participation and useful action both
need evaluation in realistic conversations. A shared installation-level attention
policy should give native and third-party ACP engines the same opportunity to
participate. Thin experiences supply facts, not competing social classifiers.
While calibrating, ambiguous fresh social events favor a bounded opportunity to
engage; silence, canceled work and spending limits remain meaningful. See the
[attention contract](../contracts/attention.v1-candidate.md).

### 5. Work has continuity and evidence

Responsibilities, accepted scope, progress, and unresolved decisions remain
discoverable when the conversation moves elsewhere. Reports identify the artifact
or change they describe. Independent verification and requester acceptance remain
distinct from the agent saying it is done. A person can redirect or stop work
without reconstructing its history.

### 6. Each individual stands on its own

An agent has its own installation, state, configuration, and service lifecycle.
Several can share a machine; others can live elsewhere and use different versions
or implementations. They coordinate through explicit interfaces and experiences.
The fleet dashboard is optional, and its unavailability does not erase an agent
or its accepted responsibilities.

### 7. Every interface can be operated for a person

Setup and everyday use should make sense to someone who does not write code.
Useful defaults come first; advanced settings remain available. Every user action
has a programmatic equivalent, including navigation, selection, drafts, and view
state. An assistant can operate the same experience while a person speaks and
watches, using the same permissions and observable results.

### 8. Open boundaries allow different implementations

Ordinary interaction uses ACP and negotiated capabilities. Optional management,
context, persona, and work contracts add features without making them prerequisites
for basic chat. Connectors own platform-specific behavior. Native agents, Codex,
and other ACP implementations should be testable against the same experiences.

### 9. Experiments improve both work and interaction

Evaluate the artifact and the conversation separately. Repeatable scenarios should
show whether agents complete the task, wait appropriately, avoid duplication, and
recover from interruption. Simulated people are labeled simulations. Their
opinions help explore behavior; they do not substitute for feedback from the
people represented.

## What success looks like

- A person meets the same individual in two experiences and can retrieve an
  authorized prior decision with its source, without copying the transcript.
- An ordinary ACP agent can join an experience without installing Constellation's
  management service or giving it access to private implementation storage.
- Work and personal agents remain isolated while their coordinator shares only
  what each relationship and destination permits.
- An agent can guide a person through setup and manipulate the visible application
  using the same operations as the human interface.
- A repeatable collaboration run preserves the exact delivered artifact, the
  separate check result, participation observations, and any human acceptance.
- Updating one installed individual preserves its identity and recoverable state
  while other individuals continue operating.

## Decisions still to test

How should concurrent conversations reconcile conflicting commitments? Which
participation rules remain useful outside carefully prompted simulations? What
should the system remember automatically, and what should require a deliberate
choice? These need experiments and human feedback rather than a fixed answer in
the initial architecture.

## Changelog

| Date | Change | Basis |
| --- | --- | --- |
| 2026-11-04 | Installation-owned attention, bounded uncertain engagement, and runtime-family lifecycle. | Explicit product direction; native/Codex compatibility and shared-room experiments. |
| 2026-11-02 | Initial draft. | The product requirements and experiments that established this family; adapted the vision/contract/status separation used by Amplifier Converge. |
