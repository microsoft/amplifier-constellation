# Experience

For human-facing applications and surfaces. An Experience lets people understand,
direct and inspect agents and work in a form suited to their setting.

## Minimal interface

Read these operations under [composition](composition.md#reading-the-minimal-interfaces).

| Operation | What callers can rely on |
| --- | --- |
| Describe | Which resources the experience can present, supported interactions and how to invoke them. |
| Read interaction state | Authorized focus, selected resources, presentation state and drafts, with a revision and relevant observation freshness. |
| Change interaction state | Open a resource, navigate, select, focus or prepare a draft through supported controls, protecting newer intent with revision checks. |

Interaction state can describe a screen, a world scene or the subject of a call.
Opening a resource or editing a draft does not send a message, commission work or
change a connection. Those actions use the corresponding owner's operations and
authorization; this separation adds no routine confirmation step.

Present human–agent communication through a [Conversation](conversation.md),
including private exchanges. Observation and configuration may use management
controls directly. The Experience owns presentation and interaction behavior;
[composition](composition.md) keeps underlying records with their owners.
[Interaction](interaction.md) owns attention, programmatic access to supported
human controls, accessible interaction and artificial agent identity. Presentation
does not establish execution, completion or authority.

## Reuse and addition

**Reuse:** The owners' conversation, agent, connection and tool APIs. Existing
[Conversations view controls](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/api.md#view-interface)
and [Observatory presentation commands](https://github.com/microsoft/amplifier-constellation-observatory/blob/main/docs/api.md)
already separate view changes from effects. Their panels and fields are examples,
not a universal UX schema.

**Constellation addition:** A semantic profile of supported controls, resource
references and interaction state. Implement it within the experience's existing
interface. Embedding, handoff links, rendered-state acknowledgments and media
controls are advertised capabilities; no shared renderer or separate service is
required. An experience may compose agent behavior without every experience
needing an agent inside it.

**Check:** Give direction in a live call, inspect the work in a visual world, and
open its original thread. Can the person recognize the same conversation and
work, join or redirect it through supported controls, and receive the outcome
without relaying internal messages?

Layout, art, navigation, voice, devices and product composition remain open.
Existing admin panels do not prescribe future UX. An experience can emphasize a
particular activity without becoming the authority for its underlying records,
and a product can combine several primitives in one application.
