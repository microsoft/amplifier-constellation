# Project page

The illustrated introduction lives at <https://microsoft.github.io/amplifier-constellation/>.
It presents four architecture illustrations and a short narrated film. The vision
and contracts remain in their canonical repository locations, linked from the page.

Build with Python 3.10 or newer; no package installation is needed:

```sh
python3 site/build.py --out _site
python3 -m http.server --directory _site 8000
```

Open `http://localhost:8000`. The build checks local references and generates the
self-contained illustration download from the same page and images. Keep edits
in the source page rather than in that generated download.

The `Project page` workflow checks pull requests and publishes changes from
`main` using GitHub Pages. Only `_site` is deployed. Repository setup uses the
GitHub Actions publishing source; no custom domain is required.

The illustrations describe architectural ideas. They are not screenshots or
evidence of deployed capabilities. The film uses synthetic narration and animated
artwork; its captions, transcript and chapter times accompany the media.

## Component pages and existing product sites

Pages in `projects/` are generated from `docs/ecosystem/registry.json`. Follow
[the map maintenance guide](../docs/ecosystem-maps.md), then run
`python3 scripts/render_ecosystem_maps.py --check` before building.
The public registry excludes private owner records.

An established product website remains the primary destination. The optional
registry field `product_site` sends the directory's main project link there and
adds a secondary **Ecosystem map** link. Its generated page is supporting
documentation with a prominent **Visit product website** link. Projects without
an established site retain their generated overview as the primary page.

The Constellation introduction at the root, the existing Converge site, and
`https://unified.amplifier.ms/` retain their existing content and ownership.
Do not replace those richer sites or their install routes with generated maps.
Review source visibility and public product copy separately: a private repository
can have approved public promotional copy, but its private map and local product
page must not be published automatically.
