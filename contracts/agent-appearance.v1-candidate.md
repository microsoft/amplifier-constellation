# Agent appearance — v1 candidate

Accepted visual convention · November 6, 2026.

An agent's character, face, clothing and accessories identify the individual.
Its portrait background identifies the installed runtime family. Model or
provider changes within one runtime do not change its family color: a native
Constellation agent using an Anthropic model still has the Constellation color.

| Installed runtime family | Background | Reference color |
| --- | --- | --- |
| Constellation native runtime | Soft sage | `#DDEBDD` |
| Codex | Lavender | `#E6DFF5` |
| Claude / Claude Agent ACP | Peach | `#F8D8C2` |
| GitHub Copilot | Sky blue | `#D5E8FA` |
| Other or unknown | Neutral gray | `#E5E7EB` |

Portrait masters are opaque square images, with the background extending to all
four corners. Do not bake in a circular crop, rounded corners, an outer white
area, or a border. Leave clipping to the destination experience. Keep the face
and distinctive accessories within a circular safe area so that the same square
asset works in round and square displays.

Use close head-and-shoulders portraits, not full-body figures. Extend the artwork
naturally to the square edges rather than placing an old circular portrait onto
a square background. Full-body references and game skins are separate assets.

These are palette anchors for illustrated backgrounds; subtle shading is allowed.
Use the colored field behind the character, with enough visible area to recognize
it in a circular crop. Do not recolor the character to indicate its runtime.
Retain the original portrait and publish a new avatar revision when updating it.

The installed agent owns the portrait. Experiences consume its current image
through the existing identity boundary, and connectors synchronize it where the
platform supports that. A runtime change should prompt a background update while
preserving the individual's recognizable design. This convention is not a new
ACP capability or a requirement that generic ACP agents provide an avatar.

Color is a helpful cue, not the only source of runtime information. Management
interfaces must also show the runtime name in text. Never infer runtime, trust,
permissions, model capabilities or connection health from an uploaded image.
The palette is a Constellation presentation convention, not vendor branding.

Minecraft skins and other spatial bodies have no portrait background. Keep their
individual character design; use the family color for preview backdrops and UI
portraits rather than painting their clothing or world surroundings to match it.
Individual deployment images and personal profiles do not belong in public
source repositories.
