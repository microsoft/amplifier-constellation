# Bring an idea into Constellation

Start with something people want to accomplish. Then identify which capabilities
the idea needs, what can be reused, and where a new contribution belongs.
You do not need to choose a repository or design an entire application first.

This is a design guide to the [shared foundation](README.md). Its examples show
how parts can work together; they are not a catalog of available integrations or
a prescription for future screens. Check the relevant implementation when
choosing the next experiment.

## What each part is for

### Experience: how a person works with the system

**The question:** “How should this feel and fit into someone's work?”

An experience lets a person ask, inspect, redirect and judge. It might live in
Slack, Teams, Minecraft, a future web or native product, or a planned Amplifier
Unified connection. Its contribution is a useful interaction, suited to that
setting, built from shared capabilities.

It takes a person's input and the relevant visible context, then presents results,
activity and choices. It preserves where a request came from and where the result
belongs. A conversation being selected, a message being sent and work being
authorized are different actions.

The experience owner can explore interaction, presentation and modality. Voice or
media transport can belong here. They rely on service operations for the shared
meaning behind those interactions; existing admin screens do not set the design.
See the [interaction agreement](../../contracts/experience.v1-candidate.md).

### Instance: the individual that stays recognizable across places

**The question:** “Which assistant is this, and what has it been entrusted with?”

Instance provides an installed agent's identity, configuration, authorized context,
responsibility records and management controls. This lets several connections
reach the same individual without each assembling another assistant from scratch.

It takes authorized connections, configuration and source references. It exposes
the individual's supported capabilities and controls, supplies permitted context
to its engine, and reports what it can observe. It keeps its own installation
state; it does not take ownership of Slack's membership or an engine's private
execution history. Recording a responsibility is also separate from starting work
or granting a domain action.

Developments in persistent identity, context access or installation management
start here. The [Instance API](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/api.md)
and [responsibility interfaces](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/responsibilities.md)
define the actual operations. An ACP client that needs only an engine can connect
directly without using Instance.

### Engine: reasoning and carrying work through

**The question:** “How does the agent decide and use its tools?”

The engine turns input and available context into reasoning, tool requests and
responses. It preserves the execution session needed to continue the work.
Native Runtime and independently chosen ACP engines are different implementations
of this role, with explicitly discovered capabilities.

It receives an ACP request, supported content and permitted tools. It returns
response and tool activity, permission requests where applicable, and a terminal
outcome for the interaction. A finished interaction is not proof that the larger
assignment succeeded. The tool's domain owns evidence of its actual effects.

Model integration, execution quality, model working-context handling and
execution-session recovery belong at this boundary. Changing the engine's
implementation must preserve its declared
interfaces; sharing ACP does not imply interchangeable saved histories. See the
[engine architecture](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/architecture.md)
and [protocol agreement](../../contracts/protocol-sdk.v1-candidate.md).

### Conversation services: a place to discuss and inspect work

**The question:** “Where can we collaborate without making everyone follow every exchange?”

Conversations supplies its own addressable rooms, participants, messages, access
rules and observable huddles. These services can support several experiences.
They let people inspect or join coordination and receive meaningful results back
where the work was requested.

They take messages, participant operations and references to the originating
request. They return durable conversation records, authorized excerpts, delivery
observations and inspectable discussion. Reading a room does not itself start
work. A huddle can be reused when it fits the request and its audience.

Slack and Teams still own their platform conversations. Their connectors deliver
into those places. Conversations can provide an additional room when useful; it
is not a required replacement or a universal store of every transcript. See the
[conversation API](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/api.md)
and [huddle interface](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/huddles.md).

### Domain connection: access to real information and actions

**The question:** “How does the agent work with the systems we already use?”

A connector translates a platform's events, identities, messages and actions into
interfaces the rest of the system can use. A tool service can expose a narrower
capability, such as reading an approved document, without becoming a chat client.

Inputs might be an authenticated platform message, a request for source material,
or an authorized action. Outputs include attributed source records, delivery
results and observations of what actually happened. The domain owns its
credentials, access checks, effects and recovery. It must distinguish a confirmed
result from an action whose outcome is still unknown.

A new business system or world capability starts here. Reuse the agent boundary
for reasoning and the shared conversation behavior for communication. See the
[repository map](../../family.toml), [saved connection agreement](../../contracts/service-connections.v1-candidate.md)
and [task authority](../../contracts/task-admission.v1-candidate.md).

### Supporting capabilities

| Need | Reusable part | What it contributes |
| --- | --- | --- |
| Shape how an agent communicates and approaches work | Persona Workshop | Portable communication and working-style profiles; applying a profile does not change permissions or identity. |
| Find out whether collaboration actually improves outcomes | ACP Lab | Repeatable experiments and observations against an independent outcome check. |
| Operate several installations conveniently | Observatory | An optional client of each installation's controls; the installations remain independently operable. |

Use the [family map](../../family.toml) to reach their owners. These capabilities
can be developed independently and used where they help.

## What the interfaces carry

An interface is an agreement about an exchange: what someone can ask for, what
comes back, and what that answer means. The shapes below are explanatory examples,
not new JSON formats or a universal message schema. Exact fields and supported
operations stay with the linked interface owner.

| Exchange | Information going in | Information coming back | Meaning to preserve |
| --- | --- | --- | --- |
| **Talk to an engine — ACP** | A supported session, input and any negotiated context/tool connections. | Response content, tool activity, permission requests and the interaction's terminal result. | A response ending does not certify the business outcome. |
| **Read or act — MCP/tool interface** | A named operation with its supported arguments, such as a document reference or game target. | Source material or an action result, with the domain's available evidence and limitations. | Reading information and changing the world are different operations. |
| **Manage one individual — Instance API** | An authenticated request to inspect or change a supported resource, with its revision where required. | The resource, a recorded operation or an explicit conflict/failure. | A saved configuration, observed running state and completed operation are different facts. |
| **Discuss and return results — conversation API** | An attributed message, conversation and intended recipients; a huddle also binds the original request. | Saved messages, delivery observations and an inspectable return path. | Saving a message is not proof that a recipient acted on it. |
| **Retrieve context — source interface** | An authorized source/conversation reference and a bounded query. | An excerpt with source identity, time/revision and coverage where supported. | Partial context stays partial; the source retains its access rules. |

For example, the Instance API publishes a descriptor of supported operations;
a client can discover controls instead of assuming every installation has them.
The Conversations message API records a request identity so a lost response need
not turn into a duplicate message. These are existing interface mechanisms, not
requirements to invent another discovery or tracking service.

Follow [ACP and tools](README.md#three-kinds-of-interface),
[Instance API](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/api.md),
[conversation API](https://github.com/microsoft/amplifier-constellation-conversations/blob/main/docs/api.md)
or [source context](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/conversation-continuity.md)
when a design needs the precise exchange.

## Where information belongs

Knowing this prevents a promising idea from accidentally creating another source
of truth that every other part must keep synchronized.

| Information | Authoritative home |
| --- | --- |
| The individual's identity, configuration and installation settings | Its Instance, when that service is used. |
| A source conversation, membership and original messages | The source platform or conversation service. |
| Model execution history and continuation state | The selected engine. |
| Responsibility or assignment | The service that accepted it under its contract. Instance can expose responsibility records; a domain can own its task and effect limits. Do not assume a shared task database. |
| Actual external state and evidence about an action | The external system owns its actual state; the connector or domain service retains action attempts, observed results and uncertainty. |
| Which sources can be retrieved in a particular interaction | The source's current authorization, applied through the context boundary. An excerpt is not a new access grant. |
| What a person sees or is drafting | The experience, using its supported state interface. Selecting or drafting does not imply sending. |

An experience may display several of these together. That does not transfer
ownership of the underlying records. The [family agreement](../../contracts/family.v1-candidate.md)
governs the separation.

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
| Better information across conversations | The source/context owners | Instance retrieval and engine context use, with original audiences preserved. |
| Quieter coordination and meaningful reports | Conversation and delivery owners | Existing participation policy, huddles and the originating connection. |
| A new end-to-end workflow | One owner for the outcome, working with affected service owners | The smallest useful combination of these parts; a new workflow need not create a new central service. |

Bring a short example: who benefits, what they should be able to accomplish, what
enters and leaves each affected part, and what observation would convince you it
works. Inspect the relevant implementation with its owner, choose the smallest
complete trial, then build and judge it. The [working guide](../working-together.md)
describes how to handle independent choices and shared changes as the idea grows.
