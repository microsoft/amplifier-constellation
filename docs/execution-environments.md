# Execution environments — design proposal

Status: proposal, November 2026. This is a design boundary, not an implemented
isolation or portability claim.

An agent is an individual. An environment is somewhere that individual can do
work. A conversation is where people and agents communicate. None of these
should be the identity or lifetime of either of the others.

## Relationships

- One agent can have assignments in several environments.
- Several agents can collaborate in one project environment.
- An installation can keep private environments alongside shared environments.
- Agents and environments can run on different machines and versions.
- Sharing project files does not automatically share credentials or authority.
- Agents use separate working copies by default, with shared folders when needed.

Separate **hosting the agent service** from **executing an assignment**. An owner
may contain a whole installation in a VM/container; independently, its work may
run in a dedicated or shared project environment. The implementation must not
force one sandbox per agent, or equate an environment with a Docker container.

## Ownership and authority

The environment owner controls mounts, network access, toolchains, resource
budgets, and which agent principals can use them. A person being allowed to talk
to an agent is not authorization to access its host or every connected account.
Conversation content, model output, repository instructions, and persona profiles
cannot change environment grants.

Confirmed baseline: a shared project repository, tools, and build caches with
separate working copies, agent principals, and credentials. Each agent can make
changes independently; integration happens through reviewed commits or pull
requests. Shared writable folders are an explicit option for work that needs them. Joining a shared project does not grant
access to another agent’s accounts. Any shared account credentials would require
a separate, explicit project-level grant. The execution boundary
must enforce these rules; a prompt saying not to use a secret is insufficient.
Shared writable project contents are intentionally shared influence. Separate
execution principals/processes and externally mediated credentials are needed if
collaborators must not be able to read each other's secrets.

## Component responsibilities

- **Instance:** persistent agent identity, connected tools, assignments, and an
  owner-facing readiness/setup flow. It selects an explicitly authorized execution
  target, not arbitrary host paths supplied in chat.
- **Runtime:** plans and uses advertised tools. Execution policy is enforced below
  the model. Selecting an external environment must not silently fall back to host
  execution when that environment is unavailable.
- **Environment component (future):** provisions and inspects execution targets,
  starts/stops bounded jobs, supplies tools and caches, brokers scoped credentials,
  supports cancellation/recovery, and produces artifact/test evidence.
- **Experience adapters:** carry conversation context and results. Slack does not
  become a GitHub credential store, development environment, or fleet scheduler.
- **Observatory:** optional UI client of these independent controls.

Standard MCP tools are a candidate execution interface across ACP agents; the
management contract is a separate, discoverable capability. No new ACP method or
mandatory shared Constellation enrollment/credential service is proposed. Each
installation remains independently usable; sharing an environment is optional.

## Owner experience

Start with “Where this agent can work”: private space, shared project space, or
connect to an existing environment. Show project name, participants, allowed
accounts/resources, and a plain readiness result. Default to a separate working
copy for each agent; make shared folders a deliberate project choice. Put images, mounts, network
rules, and other implementation choices behind advanced controls.

“Connected to GitHub” must remain distinct from “can build this project.” Readiness
should verify a checkout, dependency preparation, a representative command, and
artifact access in the actual execution target. Creating a branch or PR additionally
needs a scoped publication grant. UI actions and view state must have API/CLI parity.

## Work that can proceed before this design is settled

- Repeatable owner setup and verification of installation-owned repository tools.
- Explicit local client grants without widening unrelated remote credentials.
- Durable connector waits that preserve completed results without rerunning tools.
- Bounded, opt-in agent conversation continuation and visible delivery status.
- Preserving corrections, source edits, cancellation, and uncertain-send recovery.

Current workspace isolation stays in place while we settle the execution target,
credential broker, lifecycle, and assignment contracts. A broader network or host
filesystem grant is not a prerequisite for the setup/delivery improvements.
