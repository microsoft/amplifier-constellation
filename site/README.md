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
