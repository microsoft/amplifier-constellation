# Keep the ecosystem maps useful

Use these maps to explain what a repository does, where it fits, and which parts
can be replaced. This is a documentation convention for the
[vision](VISION.md) and [composition agreement](../contracts/composition.md),
not a new runtime protocol or an additional release-approval gate.

## Three levels of detail

1. **Ecosystem:** the five roles—Agent Instance, Conversation, Connection,
   Experience, and Tool—and their outward relationships. Keep this overview
   stable as implementations multiply.
2. **Implementation:** a named project that provides one or several roles, with
   its actual public interfaces, collaborators, and optional dependencies.
3. **Local project:** the parts of that implementation, highlighting the current
   repository. Include a small ecosystem locator so the parent context remains
   visible. A shared component may have several consumers and no single parent.

A role is not a repository, process, machine, deployment, or running instance.
Alternative implementations are peers. Constellation Instance is one Agent
Instance implementation; its internal choices do not define every Agent Instance.
Constellation Runtime is one possible engine, not a universal requirement.
An internal participant becomes another Agent Instance only when it exposes that
role and its promises. See the [Agent Instance boundary](../contracts/agent-instance.md).

## Say what each relationship means

| Label | Meaning |
| --- | --- |
| **implements** | A project supplies the named outward role or documented interface. Identify its supported scope; the label alone does not certify contract conformance. |
| **contains** | A part is inside this named project's composition or source boundary. It does not imply deployment together or shared authority. |
| **uses** | A consumer depends on a declared interface. Mark optional or alternative dependencies explicitly. |
| **supports** | Infrastructure, libraries, schemas, or checks help implement a role without themselves providing its full outward behavior. |
| **adapts** | A component translates between named interfaces. Name both ends and their relevant limitations. |
| **integrates** | A project joins independently owned components through documented interfaces. This does not imply containment or complete role conformance. |

Use labeled edges and visible project boundaries. Dashed edges distinguish
external use, support, or adaptation from internal composition. Direction means
the stated relationship, not an inferred message or control flow. Highlight every
relevant role when a product combines roles. Do not invent a sixth primitive to
accommodate a supporting library.

Map observed architecture from owner source and documentation. Keep proposals,
intended repository homes, and hypothetical alternatives visibly distinct from
available implementations. A contract describes an agreement; supported
capabilities and relevant checks establish actual compatibility. Never infer
compatibility, installability, or release readiness from a map.

## One source, several views

The canonical repository registry lives in
[`ecosystem/registry.json`](ecosystem/registry.json). `family.toml` remains the
primary repository locator. The registry records visual placement and relationships;
it must agree with the family index rather than become a competing inventory.

Only approved public records belong in the public registry. Do not export a
private repository's source, internal map, or product copy into a public site.
A private component keeps its authoritative record and maps in its own
repository until the owner authorizes their publication. Its local product page
can live at `docs/product/index.html`; this is not a public deployment or a
request to change repository visibility.

Each entry names the repository, display name, role scope, local parts and
relationships, and source references. The renderer derives output paths from the
repository name. Each component keeps
its corresponding record in `docs/ecosystem-map.json` and rendered locator/local
map in `docs/ecosystem-map.svg`. Treat that record as the local view of the
canonical registry for public components, not a separate source of architectural
truth. Private owner records remain local as described above. References should
identify the exact source revision reviewed; reader-facing links can point to
the owner's current documentation. Record uncertainty instead of completing a
plausible but unverified architecture.

The deterministic renderer at
[`scripts/render_ecosystem_maps.py`](../scripts/render_ecosystem_maps.py) produces
the repository maps. Edit the registry or renderer, then regenerate; do not patch
generated SVG labels by hand. Its `--check` mode verifies the central generated
assets and pages; its staging output supplies the corresponding component-repo
records and images without making remote edits. `--registry` selects an alternate
local input; `--local-only --staging PATH` exports private owner documentation
without producing central site files. Reuse the shared illustrated ecosystem artwork.
Keep text labels readable, colors and role order consistent, and provide useful
alternative text and a nearby plain-language description.

Typical commands from the Constellation repository:

```sh
python3 scripts/render_ecosystem_maps.py --root . --check
python3 scripts/render_ecosystem_maps.py --root . --affected microsoft/amplifier-constellation-runtime
python3 scripts/render_ecosystem_maps.py --root . --staging /path/to/map-updates
```

For a private owner's local record and product documentation:

```sh
python3 scripts/render_ecosystem_maps.py --registry /path/to/owner/docs/ecosystem-map.json --local-only --staging /path/to/private-map-updates
python3 scripts/render_ecosystem_maps.py --check-local /path/to/owner
```

The staging directory receives `<repository>/docs/…` files; review and carry them
into the corresponding checkout. `--check-local` compares the owner's record and
SVG without writing files or generating a public site. Entries require an explicit
`public` or `private` visibility. Central rendering refuses private entries by
default; the `--private-preview` override is only for an intentionally private
preview directory, never a public publication checkout. Owner-local generation
is the preferred private workflow.

A README should show its locator/local map, with the shared ecosystem overview
available in an expandable section, without becoming an architecture manual.
Product pages live together at `site/projects/<repository>/index.html` in the
Constellation site. Component README links lead to those pages; each component
does not need its own hosting pipeline. Product pages use the same map and relationships,
but lead with what people can do and how to start. Link larger implementation
maps and interface details for readers who need them. Supporting libraries should
point to the products or integrations they serve rather than advertise a fictional
standalone app.

## Update the affected maps together

When creating, renaming, splitting, moving, retiring, or changing a repository:

1. Find its registry entry and the owner documentation. Add new repositories to
   `family.toml` where appropriate and give them a stable registry identity.
2. Check whether roles, public interfaces, parents, internal parts, dependencies,
   installation routes, or names changed. Follow incoming and outgoing registry
   relationships to find affected parent, consumer, and sibling views.
3. Update the source facts, registry, and relevant descriptions. Regenerate the
   central pages/assets and stage affected repositories' local records/images.
   Carry those changes into each affected repository, including other
   repositories' maps when their depicted relationship changed. Keep README and
   product-page views in agreement.
4. Validate links, reference coverage, and generated output. Review the changed
   images at README and mobile widths, especially labels, legends, and highlights.
5. Include the affected repository list in the change. If another owner must carry
   part of the update, provide its exact patch or source reference and track that
   handoff until the published maps agree. Do not mark a coordinated update
   complete while a known stale view remains.

Pure internal changes need only update the local view when the outward role and
relationships remain the same. An actual boundary change also follows the
existing [composition](../contracts/composition.md) and
[Converge](https://github.com/microsoft/amplifier-converge) process. A documentation
update does not authorize a rollout, activation, release, or resumption of paused
product work.

## Lightweight validation

The map checks should reject duplicate or missing IDs, unknown roles, dangling
relationship targets, invalid local links, missing source references, missing
declared output files, and stale generated bytes. They should report which
reverse dependencies need regeneration. Registry entries for paused, planned,
archived, or non-installable components must retain that scope in public copy.

Run the documentation checks appropriate to the change. A map-only correction
does not itself require a product rebuild, native package qualification, or a new
deployment. Continue to respect the repository's applicable checks.
