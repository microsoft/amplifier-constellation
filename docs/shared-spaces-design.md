# Shared spaces: an everyday Constellation experience

Design proposal · November 4, 2026. This describes the next consumer experience,
not a shipped application. Existing independent installations remain usable.

## Start with a place to do something together

The home screen is a small set of spaces: a project, a household, a club, or a
private workspace. Opening a space shows the conversation, its people and agent
teammates, and the work that needs attention. A space is a social and information
boundary. It is not an execution environment or a giant shared agent transcript.

An agent can belong to several spaces and remain the same individual. Its page
answers five ordinary questions: what it helps with, what it is doing, where we
can talk to it, what it can access, and whether it needs help. Advanced engine,
connection and machine details are available behind those questions.

## The first useful journey

1. **Describe the outcome.** “Help us plan our community event.” Give the space a
   suggested editable name. Do not begin with an infrastructure inventory.
2. **Choose help.** Invite an existing teammate or create one from a short role
   description. Name, purpose and an optional picture are the primary fields.
   Put model, engine, device and advanced permissions under “Change setup.”
3. **Connect once.** The selected device's installation handles its own setup.
   Reuse an existing authorized model connection when appropriate; otherwise
   present the provider's sign-in. Show one explanation when an external system
   requires a separate approval. Never claim that a product screen removes that
   external requirement.
4. **Bring the conversation.** Use the built-in room immediately. “Also use in
   Slack” opens that installation's connector flow with the agent already chosen.
   Return to the same space after authorization, with a usable conversation link.
5. **Do a small real task.** The teammate responds, shows activity in that room,
   and returns an inspectable result. The product does not treat a green connection
   badge as successful onboarding.

A returning teammate normally needs only “Invite” and a history-access choice.
A new installation may need a model account, device consent, and platform approval.
Count these separately; do not disguise them as a universal two-click setup.

## Everyday work

The conversation remains central. A work item appears when someone accepts a
concrete responsibility, not whenever someone suggests an idea. Its compact view
shows the outcome, current owner, next action, current artifact and review state.
The person can ask “Where are we?” without having to know which service owns each
receipt. That answer links back to the source evidence available to this audience.

The default home view surfaces decisions, useful results and blocked work. Routine
tool traffic belongs in an expandable activity trail. While an agent is speaking
or acting, the current conversation shows “Thinking…” or a present-tense tool
label. A silent participation decision does not light up every room.

An interruption has a visible outcome: continue with the clarification, pause,
or cancel. Completed effects remain recorded. A changed requirement creates a new
work revision and clearly identifies any earlier review that no longer applies.
“Delivered,” “independently checked,” and “accepted” remain distinct states.

## Recovery and updates

Show one actionable problem near the affected teammate or task: “Reconnect your
model account,” “Waiting for this device,” or “Delivery needs checking.” Preserve
drafts and completed work. Technical diagnostics are expandable, with a copyable
support record that excludes secrets and private conversation contents.

An update card shows the installed version, available update and whether work is
in progress. “Update when ready” queues the installation's existing idle-safe
helper. The ordinary preference is hourly checks with installation on request.
Owners can change frequency and allow agent-initiated updates. The UI reports
which enrolled components are covered; it must not imply that every program on a
computer will be upgraded.

## Connections and audiences

The same teammate can be reached through a browser, Slack, Teams, a coding tool
or a game. Each surface retains its own conversations and presentation. Linking
an identity is not permission to merge all transcripts. A private question can
use authorized shared context without publishing its answer to the shared room.

Adding someone to a space must make history access explicit. Removing them ends
future source access. Work and personal spaces can have separate assistants;
an overarching assistant receives only the cross-space capabilities its owner
grants. Coordination authority never silently becomes unrestricted visibility.

## A voice-operated experience

Every visible action has an authenticated programmatic equivalent. This includes
opening a space, selecting a teammate, expanding activity, focusing a draft,
changing a setting, reviewing a result and navigating back. A voice assistant
uses the same revisions, permissions and validation as touch or keyboard.

The interface should keep the visible focus synchronized with the spoken task.
“Show me what blocked the builder” opens that exact item; “leave it for later”
returns to the space without losing its state. Structured confirmation is used
only for actions that actually need it, not as a substitute for understandable
defaults. Keyboard and screen-reader operation remain first-class paths.

## Component boundaries

| Concern | Existing owner / proposed composition |
| --- | --- |
| Conversations and presentation | Conversations and each external experience adapter |
| Individual identity, private profiles, responsibilities and updates | Independent Instance |
| Reasoning, model and tool execution | Installed runtime family through ACP |
| Platform installation and authorization | That installation's platform connector |
| Optional multi-device overview | Observatory, as a client of exposed controls |
| Communication and working-style preparation | Persona Workshop, with portable output |
| Execution environments | Separate design; no implementation in this proposal |

Build the consumer surface as an optional composition of those controls. It must
not require a central credential, enrollment or agent-identity service. Generic
ACP agents can join conversations with their advertised capabilities; enhanced
management controls require explicit optional discovery, with missing features
shown honestly. Do not make the runtime depend on this UI.

## Evaluation before implementation

Measure an existing-owner journey and a first-time-owner journey separately:
clicks, copied values, app switches, authorization steps, time to first useful
result, and recoveries requiring human help. Include administrator work in the
count. Also measure missed requests, duplicate replies, abandoned work, incorrect
audience sharing and uncertainty that is presented as completion.

Walk through create, invite, first task, interruption, review, reconnect and update
with keyboard, voice-driven view controls and a narrow screen. A simulated tour
is a design check; it does not substitute for a casual user's observed experience.
Use the real delivery exercise to revise this design before committing to a new
application or repository.

## First implementation slice, after design review

Use the existing Conversations service as the first visible home, with a space
record that links its room, participants and work references. Compose optional
Instance setup/connection controls rather than creating a second identity store.
Start with one owner and an existing connected model; test the first-time owner
flow separately. Keep credential entry inside the independently authenticated
installation UI. Launching an embedded setup must not disclose another owner's
control token to the consumer shell.

First validate these three journeys: make a space with one teammate and get a
real result; invite a teammate already running on another device; recover an
expired model connection without losing or repeating work. A Slack addition
should reuse the selected agent and return to the originating space. Measure the
actual external authorization steps rather than presenting a simulated sign-in
as a completed integration. No new shared provisioning service is required.

The prototype is a local concept for discussing these flows, not the rollout of
this product. The broader environment service remains a separate design.


## Finding from the first delivery exercise

A peer's review offer must not revoke a builder's ongoing tool permissions. It
changes the conversation, not the authority of the original accepted work. A new
human instruction still requires reassessment, with completed effects preserved.
Use the same distinction for progress indicators, approval cards and continuation
controls so an ordinary group discussion does not repeatedly restart work.

An invited reviewer also needs an actionable capability result: “Can inspect and
run this project” or the specific missing connection. Showing an agent in a room
does not establish that it can perform its assignment. Offer a clear handoff to
another reviewer when needed, preserving the artifact version to be checked.

Keep platform bookkeeping separate from a user’s instruction: a changed thread
reply count is not an edited request. Preserve the material message revision for
authorization and review, while recording delivery/activity metadata separately.

The completed exercise also shows why a request, a review and acceptance need
different visual states. A later request to add CI changed the reviewed version;
the independent reviewer checked the new delta and tied the result to its new
commit. The coordinator then requested acceptance. Make that sequence visible
without requiring the user to reconstruct it from scattered chat messages.

Preserve prior work when a conversation resumes: show “Continuing the existing
result” with its current artifact and remaining step, rather than starting a
second task. Completion should quiet the room; further acknowledgments should
not start another round of work or reopen unrelated earlier assignments.
