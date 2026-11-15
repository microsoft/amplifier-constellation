#!/usr/bin/env python3
"""Validate and render the canonical ecosystem registry using only the stdlib."""
import argparse
import html
import json
import re
import sys
import textwrap
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit

ROLES = ('Agent Instance', 'Conversation', 'Connection', 'Experience', 'Tool')
RELATIONS = {'contains', 'uses', 'supports', 'implements', 'adapts', 'integrates'}
REPO = re.compile(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\Z')
ID = re.compile(r'[A-Za-z0-9_-]+\Z')
REGISTRY = Path('docs/ecosystem/registry.json')
STYLE = '''body{margin:0;background:#fff7ee;color:#382316;font-family:"Trebuchet MS",system-ui,sans-serif;line-height:1.6}*{box-sizing:border-box}main,header,footer{max-width:1120px;margin:auto;padding:24px}header{border-bottom:1px solid #d7bfa8;display:flex;justify-content:space-between;gap:20px}a{color:#754121;text-underline-offset:3px}a:hover{color:#126d72}a:focus-visible,summary:focus-visible,.map-scroll:focus-visible{outline:3px solid #126d72;outline-offset:4px}h1{font-size:clamp(32px,5vw,58px);line-height:1.1;letter-spacing:-.03em;margin:30px 0 20px}h2{font-size:28px;line-height:1.25;margin-top:40px}p{max-width:75ch} .purpose{font-size:21px;max-width:65ch}.cta{display:inline-block;background:#a63c17;color:white;padding:12px 20px;border-radius:7px;font-weight:bold;text-decoration:none;margin:12px 20px 12px 0}.cta:hover{background:#803019;color:white}figure{margin:32px 0}figure img{display:block;width:100%;height:auto}.map-scroll{overflow-x:auto;overscroll-behavior-x:contain;border:1px solid #d7bfa8;border-radius:12px}.map-scroll img{min-width:960px;width:100%}.role-summary{font-size:16px}figcaption,.note{color:#6c503e;font-size:15px}details{border-block:1px solid #d7bfa8;padding:18px 0;margin:28px 0}summary{cursor:pointer;font-weight:bold}.relations{padding-left:23px}.relations li{margin-bottom:10px}.directory{list-style:none;padding:0}.directory li{border-top:1px solid #d7bfa8;padding:22px 0}.directory a{font-size:23px;font-weight:bold}.directory p{margin:6px 0}.directory .map-link{font-size:16px;font-weight:normal}.source-list{font-size:14px;overflow-wrap:anywhere}footer{border-top:1px solid #d7bfa8;font-size:14px}.skip{position:absolute;top:-100px;left:16px;background:white;padding:12px}.skip:focus{top:12px}@media(max-width:600px){main,header,footer{padding:18px}header{flex-wrap:wrap}.purpose{font-size:18px}figure{margin-inline:-6px}.cta{margin-right:8px}}'''


def fail(message):
    raise ValueError(message)


def string(value, where, maximum=400):
    if not isinstance(value, str) or not value.strip() or len(value) > maximum:
        fail(f'{where}: expected nonempty text, at most {maximum} characters')
    if any(ord(c) < 32 and c not in '\n\t' for c in value):
        fail(f'{where}: control characters are not allowed')
    return value


def url(value, where):
    string(value, where, 2000)
    p = urlsplit(value)
    if p.scheme != 'https' or not p.hostname or p.username or p.password or any(c.isspace() for c in value):
        fail(f'{where}: expected an absolute HTTPS URL without credentials')
    return value


def repo(value, where):
    if not isinstance(value, str) or not REPO.fullmatch(value) or any(x in {'.', '..'} for x in value.split('/')):
        fail(f'{where}: expected owner/repository')
    return value


def validate(data):
    if isinstance(data, dict) and 'repository' in data and 'repositories' not in data:
        data = {'repositories': [data]}
    if not isinstance(data, dict) or not isinstance(data.get('repositories'), list) or not data['repositories']:
        fail('registry: repositories must be a nonempty list')
    seen, slugs = set(), set()
    for r in data['repositories']:
        if not isinstance(r, dict):
            fail('repository entry must be an object')
        name = repo(r.get('repository'), 'repository')
        if r.get('visibility') not in {'public', 'private'}:
            fail(f'{name}: explicit visibility must be public or private')
        slug = name.split('/')[1]
        if name in seen or slug in slugs:
            fail(f'{name}: duplicate repository or output slug')
        seen.add(name); slugs.add(slug)
        for field in ('display_name', 'purpose', 'summary'):
            string(r.get(field), f'{name}.{field}', 100 if field == 'display_name' else 600)
        for field in ('roles', 'supports_roles'):
            values = r.get(field)
            if not isinstance(values, list) or any(x not in ROLES for x in values) or len(set(values)) != len(values):
                fail(f'{name}.{field}: expected unique known roles')
        nodes = r.get('nodes')
        if not isinstance(nodes, list) or not 1 <= len(nodes) <= 12:
            fail(f'{name}: expected 1–12 nodes')
        ids, self_count = set(), 0
        for n in nodes:
            if not isinstance(n, dict) or not isinstance(n.get('id'), str) or not ID.fullmatch(n['id']) or n['id'] in ids:
                fail(f'{name}: invalid or duplicate node id')
            ids.add(n['id'])
            string(n.get('label'), f'{name} node label', 100)
            if n.get('kind') not in {'self', 'internal', 'external'}:
                fail(f'{name}: invalid node kind')
            self_count += n['kind'] == 'self'
            if 'repository' in n:
                repo(n['repository'], f'{name} node repository')
            if 'link' in n and not isinstance(n['link'], bool):
                fail(f'{name}: node link must be boolean')
        if self_count != 1:
            fail(f'{name}: exactly one self node required')
        edges = r.get('edges')
        if not isinstance(edges, list) or len(edges) > 6:
            fail(f'{name}: at most six relationship rows; link a larger map for detail')
        for e in edges:
            if not isinstance(e, dict) or e.get('from') not in ids or e.get('to') not in ids or e['from'] == e['to']:
                fail(f'{name}: invalid edge endpoints')
            if e.get('relation') not in RELATIONS or ('optional' in e and not isinstance(e['optional'], bool)):
                fail(f'{name}: invalid edge relation/optional flag')
        def visit(node, trail):
            if node in trail: fail(f'{name}: containment cycle')
            for edge in edges:
                if edge['from'] == node and edge['relation'] == 'contains':
                    visit(edge['to'], trail | {node})
        for node in ids: visit(node, set())
        citations = r.get('citations')
        if not isinstance(citations, list) or not citations:
            fail(f'{name}: source citations required')
        for c in citations:
            if not isinstance(c, dict):
                fail(f'{name}: citation must be an object')
            path = string(c.get('path'), f'{name} citation path')
            if PurePosixPath(path).is_absolute() or '..' in PurePosixPath(path).parts or '\\' in path:
                fail(f'{name}: unsafe citation path')
            string(c.get('ref'), f'{name} source ref', 100)
            url(c.get('url'), f'{name} citation URL')
        cta = r.get('start_cta')
        if not isinstance(cta, dict):
            fail(f'{name}: start_cta required')
        string(cta.get('label'), f'{name} CTA label', 100); url(cta.get('url'), f'{name} CTA URL')
        if 'product_site' in r:
            site = r['product_site']
            if not isinstance(site, dict):
                fail(f'{name}: product_site must be an object')
            url(site.get('url'), f'{name} product_site URL')
            if 'label' in site:
                string(site['label'], f'{name} product_site label', 100)
        if not isinstance(r.get('notes'), list):
            fail(f'{name}: notes must be a list')
        for note in r['notes']:
            string(note, f'{name} note', 1000)
    return sorted(data['repositories'], key=lambda r: r['repository'])


def esc(value):
    return html.escape(str(value), quote=True)


def lines(value, width=31):
    return textwrap.wrap(value, width=width, break_long_words=True, break_on_hyphens=False) or ['']


def text_svg(value, x, y, width=31, size=18, fill='#382316', anchor='middle'):
    return ''.join(f'<text x="{x}" y="{y+i*23}" text-anchor="{anchor}" font-size="{size}" fill="{fill}">{esc(line)}</text>' for i, line in enumerate(lines(value, width)))


def map_svg(r):
    """Draw each node once, grouping internal code and external collaborators."""
    nodes = {n['id']: n for n in r['nodes']}
    own = next(n for n in r['nodes'] if n['kind'] == 'self')
    internal = [n for n in r['nodes'] if n['kind'] == 'internal']
    external = [n for n in r['nodes'] if n['kind'] == 'external']
    # Containment depth gives a stable top-down local topology. Unconnected owned
    # parts retain one level below the repository without inventing an edge.
    depth = {own['id']: 0}
    for _ in nodes:
        for e in r['edges']:
            if e['relation'] == 'contains' and e['from'] in depth and nodes[e['to']]['kind'] == 'internal':
                depth[e['to']] = max(depth.get(e['to'], 1), depth[e['from']] + 1)
    levels = {}
    for n in internal:
        levels.setdefault(depth.get(n['id'], 1), []).append(n)
    positions = {own['id']: (170, 310, 360, 94)}
    y = 482
    for level in sorted(levels):
        values = levels[level]
        for start in range(0, len(values), 2):
            row = values[start:start + 2]
            for col, n in enumerate(row):
                positions[n['id']] = ((65 + col * 305) if len(row) == 2 else 218, y, 270, 94)
            y += 172
    boundary_bottom = max(positions[n['id']][1] + positions[n['id']][3] for n in [own] + internal) + 70
    for i, n in enumerate(external):
        positions[n['id']] = (860, 310 + i * 144, 300, 94)
    bottom = max(boundary_bottom, max((p[1] + p[3] for p in positions.values()), default=404) + 35)
    height = bottom + 76
    relation_description = ' '.join(nodes[e['from']]['label'] + ' ' + e['relation'] + (' optionally' if e.get('optional') else '') + ' ' + nodes[e['to']]['label'] + '.' for e in r['edges'])
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 {height}" role="img" aria-labelledby="title desc">',
             f'<title id="title">{esc(r["display_name"])}: ecosystem and project map</title>',
             f'<desc id="desc">{esc(r["summary"])} Role highlights show scope, not certification. Each project or part appears once. Internal parts are grouped in this repository; collaborators remain outside. {esc(relation_description)}</desc>',
             '<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L10 5L0 10" fill="none" stroke="#76523b" stroke-width="1.5"/></marker></defs>',
             f'<rect width="1200" height="{height}" rx="18" fill="#fff7ee"/>',
             '<g font-family="Trebuchet MS,Arial,sans-serif">',
             text_svg('Constellation · ecosystem locator', 35, 36, 95, 20, anchor='start')]
    for i, role in enumerate(ROLES):
        x = 30 + i * 235
        direct, support = role in r['roles'], role in r['supports_roles']
        fill, stroke = ('#3e6753', '#3e6753') if direct else ('#fff7ee', '#a63c17' if support else '#d7bfa8')
        dash = ' stroke-dasharray="7 4"' if support and not direct else ''
        parts += [f'<rect x="{x}" y="58" width="210" height="58" rx="12" fill="{fill}" stroke="{stroke}" stroke-width="2"{dash}/>', text_svg(role, x + 105, 94, 22, 18, '#fff7ee' if direct else '#382316')]
    parts += [text_svg('Filled: outward role · Dashed: supports role · Unmarked: context', 35, 145, 120, 16, '#6c503e', 'start'),
              text_svg(r['display_name'], 35, 201, 80, 28, anchor='start'),
              text_svg('One repository, its internal parts, and the interfaces around it', 35, 234, 110, 17, '#6c503e', 'start'),
              f'<rect data-project-boundary="true" x="30" y="264" width="665" height="{boundary_bottom - 264}" rx="18" fill="#f9ecdc" stroke="#d7bfa8" stroke-width="2"/>',
              text_svg('This repository · internal parts', 52, 290, 70, 15, '#6c503e', 'start')]
    if external:
        parts.append(text_svg('Other projects / interfaces', 1010, 290, 45, 15, '#6c503e'))

    # Route inter-project edges in the clear gutter; local edges use the space
    # between containment levels. Arrowheads always point to the stated target.
    labeled_contains = set()
    edge_labels = []
    for i, e in enumerate(r['edges']):
        a, b = nodes[e['from']], nodes[e['to']]
        ax, ay, aw, ah = positions[a['id']]; bx, by, bw, bh = positions[b['id']]
        relation = e['relation'] + (' · optional' if e.get('optional') else '')
        if a['kind'] != 'external' and b['kind'] != 'external':
            sx, sy, tx, ty = ax + aw / 2, ay + ah, bx + bw / 2, by
            if ty > sy:
                # Branch immediately above the destination row. The repository
                # trunk stays between columns instead of crossing earlier parts.
                middle = ty - 30
                path = f'M{sx} {sy}V{middle}H{tx}V{ty - 6}'
                lx, ly = (sx + tx) / 2, middle - 10
                if sx == tx: lx += 57
            elif sx == tx and ay > by + bh:
                # A same-column upward relationship fits in the free row gap;
                # taking it through the outside gutter would cross sibling parts.
                path = f'M{sx} {ay}V{by + bh + 6}'
                lx, ly = sx + 57, (ay + by + bh) / 2
            else:
                # Non-containment local relations may point upward or sideways.
                gutter = 710 + i * 8
                path = f'M{ax + aw} {ay + ah/2}H{gutter}V{by + bh/2}H{bx + bw + 6}'
                lx, ly = gutter, min(ay, by) - 12
        elif a['kind'] != 'external' and b['kind'] == 'external':
            sx, sy = ax + aw / 2, ay + ah
            gutter = 720 + i * 12
            lane = sy + 17 + i * 6
            ty = by + bh / 2
            path = f'M{sx} {sy}V{lane}H{gutter}V{ty}H{bx - 6}'
            lx, ly = 778, ty - 10
        elif a['kind'] == 'external' and b['kind'] != 'external':
            sy = ay + ah / 2
            gutter = 720 + i * 12
            lane = by - 18 - i * 5
            tx = bx + bw / 2
            path = f'M{ax} {sy}H{gutter}V{lane}H{tx}V{by - 6}'
            lx, ly = 778, sy - 10
        else:
            sy, ty = ay + ah / 2, by + bh / 2
            gutter = 720 + i * 12
            path = f'M{ax} {sy}H{gutter}V{ty}H{bx - 6}'
            lx, ly = 778, sy - 10
        dash = '' if e['relation'] == 'contains' and a['kind'] != 'external' and b['kind'] != 'external' else ' stroke-dasharray="7 5"'
        parts.append(f'<g data-edge-from="{esc(a["id"])}" data-edge-to="{esc(b["id"])}" data-relation="{esc(e["relation"])}"><path d="{path}" fill="none" stroke="#76523b" stroke-width="2" marker-end="url(#arrow)"{dash}/></g>')
        # One label per containment source makes a shared fan-out readable.
        if e['relation'] == 'contains' and a['kind'] != 'external' and b['kind'] != 'external':
            if a['id'] in labeled_contains:
                continue
            labeled_contains.add(a['id'])
            lx, ly = ax + aw / 2, ay + ah + 25
        # Labels have a paper backing so crossings do not obscure the words.
        label_width = min(160, 15 + len(relation) * 8)
        edge_labels += [f'<rect x="{lx-label_width/2}" y="{ly-17}" width="{label_width}" height="23" rx="5" fill="#fff7ee"/>', text_svg(relation, lx, ly, 24, 15)]

    parts.extend(edge_labels)
    for n in r['nodes']:
        x, y, width, h = positions[n['id']]
        own_node = n['kind'] == 'self'
        label_lines = lines(n['label'], 30 if own_node else 25)
        # Existing validated labels are bounded; wrap rather than truncate them.
        size = 18 if len(label_lines) <= 2 else 16
        label_y = y + (h - (len(label_lines)-1)*23) / 2 - 3
        url_value = 'https://github.com/' + n['repository'] if n.get('repository') and n.get('link', True) else None
        parts.append(f'<g data-node-id="{esc(n["id"])}" data-kind="{esc(n["kind"])}">')
        if url_value: parts.append('<a href="'+esc(url_value)+'" target="_top">')
        parts += [f'<rect x="{x}" y="{y}" width="{width}" height="{h}" rx="12" fill="{"#f4cfb5" if own_node else "#fffdf9"}" stroke="{"#a63c17" if own_node else "#d7bfa8"}" stroke-width="{3 if own_node else 1.5}"/>', text_svg(n['label'], x + width/2, label_y, 30 if own_node else 25, size), text_svg({'self':'This repository','internal':'Internal part','external':'External project / interface'}[n['kind']], x + width/2, y+h-10, 45, 12, '#6c503e')]
        if url_value: parts.append('</a>')
        parts.append('</g>')
    parts += [text_svg('Solid: internal composition · Dashed: external use or support · Arrow follows the stated relationship', 35, height-27, 135, 15, '#6c503e', 'start'), '</g></svg>\n']
    return ''.join(parts)


def document(title, body, style='../../projects/style.css'):
    return f'<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{esc(title)}</title><link rel="stylesheet" href="{style}"></head><body><a class="skip" href="#main">Skip to content</a><header><a href="../../index.html">Amplifier Constellation</a><a href="../index.html">Explore projects</a></header><main id="main">{body}</main><footer><a href="https://github.com/microsoft/amplifier-constellation">Source and ecosystem guide</a></footer></body></html>\n'


def product_page(r):
    nodes = {n['id']:n for n in r['nodes']}
    relationships = ''.join('<li>'+esc(nodes[e['from']]['label'])+' <strong>'+esc(e['relation'])+'</strong> '+esc(nodes[e['to']]['label'])+(' (optional)' if e.get('optional') else '')+'.</li>' for e in r['edges'])
    notes = ''.join('<li>'+esc(n)+'</li>' for n in r['notes'])
    citations = ''.join('<li><a href="'+esc(c['url'])+'">'+esc(c['path'])+'</a> · reviewed '+esc(c['ref'])+'</li>' for c in r['citations'])
    site = r.get('product_site')
    title = r['display_name']+' — ecosystem map' if site else r['display_name']
    cta = {'url':site['url'],'label':site.get('label','Visit product website')} if site else r['start_cta']
    body = f'<h1>{esc(title)}</h1><p class="purpose">{esc(r["purpose"])}</p>'
    if site:
        body += '<p>This supporting page shows where the project fits in Constellation. Visit its product website for the full introduction and ways to get started.</p>'
    body += f'<a class="cta" href="{esc(cta["url"])}">{esc(cta["label"])}</a>'
    if site and r['start_cta']['url'] != site['url']:
        body += f'<a href="{esc(r["start_cta"]["url"])}">{esc(r["start_cta"]["label"])}</a> · '
    body += f'<a href="https://github.com/{esc(r["repository"])}">Repository and documentation →</a>'
    role_text = ('Outward roles: '+', '.join(r['roles'])+'. ') if r['roles'] else 'Supporting component. '
    if r['supports_roles']: role_text += 'Supports: '+', '.join(r['supports_roles'])+'.'
    body += f'<h2>Where it fits</h2><p>{esc(r["summary"])}</p><p class="role-summary">{esc(role_text)}</p><figure><div class="map-scroll" role="region" tabindex="0" aria-label="Scrollable project map"><a href="map.svg"><img src="map.svg" alt="{esc(r["display_name"])}: ecosystem roles and project relationships"></a></div><figcaption>Scroll the map or <a href="map.svg">open it full size</a>. Role highlights describe scope, not proof of complete contract conformance.</figcaption></figure>'
    if relationships: body += '<ul class="relations">'+relationships+'</ul>'
    body += '<details><summary>The five Constellation building blocks</summary><figure><img src="../../assets/art/building-blocks.png" alt="Experience presents Conversation; Agent Instance participates through Connection and uses Tool."><figcaption>Many implementations can fulfill each role. Projects choose their own internals.</figcaption></figure><a href="https://github.com/microsoft/amplifier-constellation/blob/main/contracts/README.md">Read the boundary agreements →</a></details>'
    body += '<details><summary>Requirements, map sources, and implementation details</summary>'+('<ul class="note">'+notes+'</ul>' if notes else '')+'<ul class="source-list">'+citations+'</ul></details>'
    return document(title,body)


def outputs(root, entries, staging=None, local_only=False, private_preview=False):
    if not local_only and not private_preview and any(r.get('visibility') == 'private' for r in entries):
        fail('private records cannot enter the public site; use --local-only or an explicit --private-preview in a private staging directory')
    result = {root/'site/projects/style.css': STYLE+'\n'}
    local = {}
    directory=[]
    for r in entries:
        slug=r['repository'].split('/')[1]; svg=map_svg(r)
        result[root/'site/projects'/slug/'index.html']=product_page(r)
        result[root/'site/projects'/slug/'map.svg']=svg
        if staging:
            local[staging/slug/'docs/ecosystem-map.svg']=svg
            local[staging/slug/'docs/ecosystem-map.json']=json.dumps(r,indent=2,ensure_ascii=False,sort_keys=True)+'\n'
            if local_only:
                page=product_page(r).replace('../../projects/style.css','style.css').replace('../../index.html','https://microsoft.github.io/amplifier-constellation/').replace('../index.html','https://microsoft.github.io/amplifier-constellation/projects/').replace('../../assets/art/building-blocks.png','https://raw.githubusercontent.com/microsoft/amplifier-constellation/main/site/assets/art/building-blocks.png')
                local[staging/slug/'docs/product/index.html']=page
                local[staging/slug/'docs/product/style.css']=STYLE+'\n'
                local[staging/slug/'docs/product/map.svg']=svg
        site=r.get('product_site'); primary=site['url'] if site else slug+'/index.html'
        map_link=f'<a class="map-link" href="{esc(slug)}/index.html">Ecosystem map</a>' if site else ''
        directory.append(f'<li><a href="{esc(primary)}">{esc(r["display_name"])}</a><p>{esc(r["purpose"])}</p>{map_link}</li>')
    result[root/'site/projects/index.html']=document('Explore Constellation projects','<h1>Find your starting point.</h1><p class="purpose">Explore applications, connections, and the components that help build them. Each project explains its place and how to get started.</p><ul class="directory">'+''.join(directory)+'</ul>','style.css').replace('href="../../index.html"','href="../index.html"').replace('href="../index.html">Explore projects','href="index.html">Explore projects')
    return local if local_only else {**result, **local}


def affected(entries, changed):
    known={r['repository'] for r in entries} | {n['repository'] for r in entries for n in r['nodes'] if n.get('repository')}
    if not set(changed)<=known: fail('affected: unknown repository')
    dependents={name:set() for name in known}
    for r in entries:
        owner=r['repository']
        nodes={n['id']:n.get('repository') or (owner if n['kind'] in {'self','internal'} else None) for n in r['nodes']}
        # A changed reference also changes the map in which it is displayed.
        for n in r['nodes']:
            if n.get('repository'): dependents[n['repository']].add(owner)
        for edge in r['edges']:
            source,target=nodes[edge['from']],nodes[edge['to']]
            if source is None or target is None: continue
            relation=edge['relation']
            if relation in {'uses','contains','implements'}: dependents[target].add(source)
            elif relation=='supports': dependents[source].add(target)
            else:  # Adapters and integrations can require both owners to update.
                dependents[source].add(target); dependents[target].add(source)
    impacted=set(changed); pending=list(changed)
    while pending:
        for consumer in dependents[pending.pop()]-impacted:
            impacted.add(consumer); pending.append(consumer)
    return sorted(impacted)


def main(argv=None):
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root',type=Path,default=Path.cwd()); p.add_argument('--staging',type=Path)
    p.add_argument('--registry',type=Path,help='Alternate registry input; defaults to ROOT/docs/ecosystem/registry.json')
    p.add_argument('--local-only',action='store_true',help='Export only owner-local docs/product and map files; requires --staging')
    p.add_argument('--private-preview',action='store_true',help='Explicitly permit private records in a private preview root; never use a publication checkout')
    p.add_argument('--check-local',type=Path,help='Read-only check of one owner checkout docs/ecosystem-map.json and SVG, with no site output')
    p.add_argument('--check',action='store_true'); p.add_argument('--affected',nargs='+')
    args=p.parse_args(argv)
    try:
        if args.local_only and not args.staging: fail('--local-only requires --staging')
        source=args.registry or (args.check_local/'docs/ecosystem-map.json' if args.check_local else args.root/REGISTRY)
        entries=validate(json.loads(source.read_text(encoding='utf-8')))
        if args.affected:
            print('\n'.join(affected(entries,args.affected))); return 0
        if args.check_local:
            if len(entries) != 1: fail('--check-local requires exactly one repository record')
            files={args.check_local/'docs/ecosystem-map.svg':map_svg(entries[0]),args.check_local/'docs/ecosystem-map.json':json.dumps(entries[0],indent=2,ensure_ascii=False,sort_keys=True)+'\n'}
        else:
            files=outputs(args.root,entries,args.staging,args.local_only,args.private_preview)
        if args.check or args.check_local:
            drift=[str(path) for path,body in files.items() if not path.is_file() or path.read_bytes()!=body.encode('utf-8')]
            if not args.check_local and not args.local_only:
                project_root=args.root/'site/projects'
                actual=set(project_root.glob('*/map.svg')) | set(project_root.glob('*/index.html'))
                drift += [str(path)+' (unregistered output)' for path in sorted(actual-set(files))]
            if drift: print('Generated map drift:\n'+'\n'.join(drift),file=sys.stderr); return 1
            print(f'PASS: {len(entries)} repositories, {len(files)} generated files'); return 0
        for path,body in files.items():
            path.parent.mkdir(parents=True,exist_ok=True); path.write_text(body,encoding='utf-8')
        print(f'Rendered {len(entries)} repositories, {len(files)} files'); return 0
    except (ValueError,OSError,KeyError,TypeError) as exc:
        print(f'Map validation failed: {exc}',file=sys.stderr); return 2


if __name__=='__main__':
    raise SystemExit(main())
