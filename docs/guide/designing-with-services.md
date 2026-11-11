# Bring an idea into Constellation

Start with something people want to accomplish. Then identify which capabilities
the idea needs, what can be reused, and where a new contribution belongs.
You do not need to choose a repository or design an entire application first.

This is a design guide to the [shared foundation](README.md). Its examples show
how parts can work together; they are not a catalog of available integrations or
a prescription for future screens. Check the relevant implementation when
choosing the next experiment.

## Find the agreement behind your question

| What you need to understand | Authoritative home |
| --- | --- |
| What is the value we are trying to create? | [Vision](../VISION.md) |
| Which part owns identity, reasoning, conversations or domain access? | [Family responsibilities](../../contracts/family.v1-candidate.md#responsibilities-and-connections) |
| Where do Companion, Unified, shared platforms and device clients fit? | [Planned product, platform and client boundaries](../../contracts/family.v1-candidate.md#planned-products-platforms-and-clients) |
| What keeps conversations, execution history, assignments and effects distinct? | [Information ownership](../../contracts/family.v1-candidate.md#information-ownership) |
| What do ACP, MCP and service APIs carry; who owns voice/media transport? | [Interface responsibilities](../../contracts/family.v1-candidate.md#interface-responsibilities) |
| What must follow a request through execution and delivery? | [Interaction agreement](../../contracts/experience.v1-candidate.md) |
| What are the actual operations and payloads? | [Interface-owner references](../../contracts/README.md#go-to-the-interface-owner-for-details) |
| How do people make decisions and change direction together? | [Developing the family](../../contracts/family.v1-candidate.md#developing-the-family), illustrated in [Working together](../working-together.md) |

These agreements are the source of the roles and meanings illustrated here and in
[the visual tour](index.html). The guide is a reading aid: deleting it would not
remove a requirement. A changed commitment belongs in its authoritative home,
with its affected examples reviewed in the same change.

## Place the idea at the right level

Companion and Unified are product experiences. A product owner decides how people
ask, inspect, redirect and receive results. The planned client repositories supply
reusable support for web, Windows, macOS, Linux, iOS and Android; choosing one does
not create another agent identity or move domain permissions into the client.

The planned platform repositories give products places to reuse work organization,
resources and operations, canvas and publishing, portability, realtime media and
context access. An idea can deepen one of those capabilities without rebuilding
an entire product. These are intended ownership homes, not claims that their
interfaces or integrations are available. Use the [family agreement](../../contracts/family.v1-candidate.md#planned-products-platforms-and-clients)
for the boundaries and [repository map](../../family.toml) for the homes.

For example, Companion's shared board could use Canvas while Companion owns its
call experience. A quiet work huddle still uses Conversations or a suitable source
platform; the product returns the meaningful result where the person asked. Source
records keep their original audience, and a scheduled operation still needs the
authority required by its engine and domain.

## Work through an idea: a launch-readiness brief

“Use these approved notes to prepare a launch-readiness brief. Highlight unresolved
decisions, consult the relevant specialists, and report back in this Slack thread.”

This is an illustrative design exercise. It assumes authorized note-reading tools,
participating agents and a suitable coordination destination are available for
the trial; it does not claim a packaged launch workflow exists.

| Moment | Who does what | What the person gains |
| --- | --- | --- |
| Request | The Slack connector retains the author, source and return thread. Instance connects the request to the intended individual; the client asks the engine to work. | One request has a clear recipient and destination. |
| Read | The engine uses permitted source tools. Each source returns the approved material with its provenance and coverage. | The brief can explain which information supports it and what is missing. |
| Coordinate | Participating agents use an appropriate existing conversation or supported huddle. Each keeps its own accepted work and tools. | Specialist discussion is available to inspect or join without filling the original thread. |
| Correct | The person changes the launch date. The connector retains this as a new source; host and engine use supported input continuity, or the next supported boundary. | Useful work continues with the correction; old evidence is not rewritten. |
| Return | The agent reports the brief, unresolved decisions and evidence. The connector delivers to the original thread and records the delivery outcome. | The person gets a useful result and can decide what to do next. Preparing the brief does not approve the launch. |

The combined check asks whether the corrected date appears in the brief, sources
remain accessible to the intended audience, unresolved decisions stay explicit,
coordination remains inspectable, and the report reaches the right thread once.
A collection of passing component checks would not answer all of those questions.

Now suppose the idea expands: external partners should see a version of the brief.
That affects audience and source access, not just layout. The affected owners agree
what may be shared and how it is produced. In contrast, a clearer internal layout
within the same promises can be tried by the experience owner independently.

## Find the starting point for your idea

| If the idea is… | Start with… | Reuse or coordinate with… |
| --- | --- | --- |
| A better way for people to inspect, ask or redirect | An experience | Instance and conversation controls, domain evidence and delivery. |
| Access to a new system or a useful new action | A connector or tool service | The system's identity/access model, ACP engine and existing source/delivery paths. |
| Better reasoning, tool use or long-work continuation | The engine | Provider integration and the domain's actual outcome evidence. |
| Better information across conversations | The source/context owners, including the planned Context capability | Existing source access, Instance retrieval and engine context use, with original audiences preserved. |
| Quieter coordination and meaningful reports | Conversation and delivery owners | Existing participation policy, huddles and the originating connection. |
| A reusable shared board, live-media path or work platform capability | The relevant planned platform owner | Product experiences and existing serving owners; agree the interface before depending on it. |
| A new end-to-end workflow | One owner for the outcome, working with affected service owners | The smallest useful combination of these parts; a new workflow need not create a new central service. |

Bring a short example: who benefits, what they should be able to accomplish, what
enters and leaves each affected part, and what observation would convince you it
works. Inspect the relevant implementation with its owner, choose the smallest
complete trial, then build and judge it. The [working guide](../working-together.md)
describes how to handle independent choices and shared changes as the idea grows.
