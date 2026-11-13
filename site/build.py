#!/usr/bin/env python3
"""Build the static project page and a self-contained illustration download."""
from pathlib import Path
from html import escape
from html.parser import HTMLParser
from urllib.parse import unquote, urlsplit
import argparse
import base64
import json
import re
import shutil

SOURCE = Path(__file__).resolve().parent
PUBLIC_URL = 'https://microsoft.github.io/amplifier-constellation/'


class References(HTMLParser):
    def __init__(self):
        super().__init__()
        self.refs = []
        self.ids = set()

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if 'id' in values:
            if values['id'] in self.ids:
                raise ValueError(f'Duplicate HTML id: {values["id"]}')
            self.ids.add(values['id'])
        self.refs.extend(values[k] for k in ('src', 'href', 'poster') if values.get(k))


def validate(folder, page):
    parser = References()
    parser.feed(page)
    for ref in parser.refs:
        url = urlsplit(ref)
        if url.scheme or url.netloc:
            continue
        if not url.path:
            if url.fragment and url.fragment not in parser.ids:
                raise ValueError(f'Missing anchor: {ref}')
            continue
        path = (folder / unquote(url.path)).resolve()
        if not path.is_relative_to(folder.resolve()) or not path.exists():
            raise ValueError(f'Missing or escaping local reference: {ref}')


def build(destination):
    destination = destination.resolve()
    if destination == SOURCE or SOURCE.is_relative_to(destination):
        raise ValueError('Build into a separate output directory.')
    destination.mkdir(parents=True, exist_ok=True)
    for name in ('style.css', 'site.js'):
        shutil.copyfile(SOURCE / name, destination / name)
    shutil.copytree(SOURCE / 'assets', destination / 'assets', dirs_exist_ok=True)
    info = json.loads((SOURCE / 'assets/media/chapters.json').read_text())
    if len(info['chapters']) != 4:
        raise ValueError('The introduction must contain four chapters.')
    buttons = []
    for chapter in info['chapters']:
        start = chapter['start']
        label = f'{int(start)//60}:{int(start)%60:02}'
        buttons.append(f'<button type="button" data-time="{start}" aria-label="Go to {escape(chapter["title"])}"><span>{label}</span>{escape(chapter["title"])}</button>')
    page = (SOURCE / 'index.html').read_text().replace('@@CHAPTERS@@', '\n'.join(buttons))
    (destination / 'index.html').write_text(page)
    (destination / '.nojekyll').write_text('')
    # The download is derived from this same page; it is not a second authored guide.
    portable = re.sub(r'\s*<!-- film-start -->.*?<!-- film-end -->', '', page, flags=re.S)
    portable = portable.replace('<link rel="stylesheet" href="style.css">', '<style>' + (SOURCE / 'style.css').read_text() + '</style>')
    portable = portable.replace('<script src="site.js" defer></script>', '')
    portable = portable.replace('href="#film"', f'href="{PUBLIC_URL}#film"')
    portable = portable.replace('href="./"', f'href="{PUBLIC_URL}"')
    portable = portable.replace('<a href="downloads/constellation-illustrated.html" download>Take the illustrations with you</a>', '')
    portable = re.sub(r'<a class="art"[^>]*>(.*?)</a>', r'<div class="art">\1</div>', portable, flags=re.S)
    portable = portable.replace('Open any illustration to see it at full size.', 'All four illustrations are embedded in this file for offline viewing.')
    for image in sorted((SOURCE / 'assets/art').glob('*.png')):
        data = base64.b64encode(image.read_bytes()).decode('ascii')
        portable = portable.replace('src="assets/art/' + image.name + '"', 'src="data:image/png;base64,' + data + '"')
    downloads = destination / 'downloads'
    downloads.mkdir(exist_ok=True)
    (downloads / 'constellation-illustrated.html').write_text(portable)
    validate(destination, page)
    validate(downloads, portable)
    if '@@' in page:
        raise ValueError('Unresolved template placeholder.')
    print(f'Built project page and four-image offline download in {destination}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    build(parser.parse_args().out)
