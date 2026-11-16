# Approvals across an agent’s chats

An agent should bring an action approval to the person who can decide, wherever
that person is working. The person can approve the current request, remember a
matching choice across that agent’s chats, or explicitly choose automatic
approval. A pending approval should be visible as a decision to make, with its
actual scope, rather than unexplained thinking.

This guide explains the optional approval ownership agreement in
[ACP hosting](../contracts/profiles/acp-hosting.md#approvals). It does not add
mandatory ACP methods or prescribe every Agent Instance’s architecture. Exact
fields and executable behavior belong to the serving implementations:

- [Instance approval policy](https://github.com/microsoft/amplifier-constellation-instance/blob/main/docs/approval-policy.md)
  owns policy controls, client delegation, durable rules and current-request decisions.
- [Runtime optional capabilities](https://github.com/microsoft/amplifier-constellation-runtime/blob/main/docs/acp-optional-capabilities.md#optional-host-owned-approval-rules)
  owns its negotiation, trusted semantic descriptors and execution checks.
- Other engines implement or decline the same optional profile. Advertised
  support and a combined check determine compatibility; an index entry is not
  evidence of an installed version’s support.

## One owner for the policy

For an Instance-hosted agent, Instance owns the installed agent’s policy across
its chats. The selected engine owns execution authority. A connector presents
and returns the exact offered choices, preserving the authenticated caller,
request and conversation; it does not create another remembered-approval cache.
A fleet interface is an optional management client, not a required permission
or identity service.

The installation starts in **Ask** with no clients delegated to remember choices.
Upgrading or adding a connector does not silently switch policy. An owner chooses
which existing authenticated clients may create remembered rules. Human names,
quoted messages, a model’s arguments and display labels cannot confer that
authority. Ordinary approval of the current request does not need permission to
create rules for future chats.

## Make each choice’s scope clear

| Choice | Meaning |
| --- | --- |
| Allow the current request | Select the engine’s current offered permission. If this establishes a conversation grant, disclose that lifetime rather than promising one isolated effect. |
| Always allow matching requests | Save a rule with this installed agent’s policy owner and apply it only to later trusted requests with the same bound semantics. It is not permission for every tool with a similar name. |
| Automatically approve permitted actions | Explicitly select the current valid action permission when it has one unambiguous Allow once option. This does not require reusable matching support and does not create remembered rules. |
| Revoke a rule | Stop that rule authorizing future matches. This does not undo an accepted action or revoke a grant already retained by the engine. |

Automatic approval can approve an offered file-access, configuration or
conversation grant within the engine’s limits. It does not bypass hard engine,
resource, operating-system, budget or cancellation checks. Questions and sign-in
flows remain their own interactions. Existing separately authorized background
work does not gain new authority from this foreground setting.

In Ask mode, revoked matches ask again. If Automatic remains selected, it still
approves valid current offers independently of those rules. Select Ask to stop
that future automatic approval; use the engine’s supported Stop or grant-revocation
control for already accepted work. Do not describe either action as undoing effects.

## Match the resolved request, not its wording

The optional `com.microsoft.amplifier.approval-policy` version 1 profile first
negotiates the policy owner with the selected engine. A supporting engine supplies
a trusted producer identity and an opaque semantic key for each reusable request,
plus a bounded description, the current Allow once option and authority lifetime.
The serving documents above define the exact placement, fields and validation.
An unsupported or malformed descriptor cannot enable the host’s Always choice.
Existing engine-local controls remain governed by that engine; do not claim that
a host rule or its revocation manages an unnegotiated engine cache.

The engine derives the key from the resolved action and its effective scope:
arguments and resources, current workspace and grants, relevant tool/schema and
mode, and actual MCP endpoint/transport/configuration where applicable. A server
label, tool alias, human explanation or UUID alone is not authority. Changed
requested content, endpoint or effective scope must not reuse an approval for the
old request. This binds configured semantics, not a lease against arbitrary
filesystem writes or changes at a remote server; resource checks still apply.
The description explains the choice without carrying secrets; it is not a match key.

The policy owner also binds a rule to the installed agent and selected backend
configuration and producer. Chats and transient callback IDs are not the rule’s
scope, and a rule is not portable authorization for another agent. Persist only
bounded summaries and opaque bindings, not raw tool arguments or credentials.

There is one remembered-rule store. After a valid host Always choice, the host
records the rule and answers the original live request with its existing Allow
once option. The engine does not silently create a second persistent approval.

## Preserve control through interruption

A decision belongs to the exact current authenticated request, active prompt,
verified source and offered option. Policy revisions, delegation and current
bindings are checked at the decision boundary. Expired or revoked issuing
credentials cannot keep authorizing new matches. Stale or duplicate responses,
changed source bindings, cancellation and old callback IDs cannot approve new work.

Persist authorization before forwarding it, without claiming that it was delivered
or executed. Losing the reply or transport leaves its outcome unknown; it does
not authorize an automatic replay. Durable rules can survive restart, but an old
pending callback cannot become a fresh decision. Owners retain supported controls
for inspection and revocation, including their limits and retention behavior.

## Check the composition

Use the actual host, engine and connector boundaries: approve once, remember one
request, reuse it in a second chat and after restart, change the effect or endpoint,
then revoke it. Verify that an undelegated client can approve the current request
without creating a rule. Exercise a revoked credential, changed policy revision,
Stop, duplicate and late reply, disconnect and failed durable write. Check that
Automatic remains explicit, questions are still presented, hard engine limits
still refuse, and the same rules do not authorize another installed agent.

A unit test of a descriptor is useful evidence about the producer. It does not
by itself establish that a person sees and can answer the approval in Slack or
another experience. Return that combined outcome and its limits to the user.
