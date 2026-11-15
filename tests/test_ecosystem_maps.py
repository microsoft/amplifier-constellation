import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import re
import tempfile
import unittest
import xml.etree.ElementTree as ET

SOURCE=Path(__file__).resolve().parents[1]/'scripts/render_ecosystem_maps.py'
spec=importlib.util.spec_from_file_location('ecosystem_renderer',SOURCE)
m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m)


def record(name='microsoft/example'):
    return {'repository':name,'visibility':'public','display_name':'Example & Agent','purpose':'A test application.',
            'roles':['Agent Instance'],'supports_roles':['Tool'],'summary':'Uses an optional engine.',
            'nodes':[{'id':'self','label':'Example & Agent','kind':'self','repository':name},
                     {'id':'engine','label':'Engine <alternative>','kind':'external','repository':'microsoft/engine'}],
            'edges':[{'from':'self','to':'engine','relation':'uses','optional':True}],
            'citations':[{'path':'README.md','ref':'a'*40,'url':'https://github.com/'+name+'/blob/main/README.md'}],
            'start_cta':{'label':'Read the guide','url':'https://github.com/'+name},'notes':['Illustrative fixture; no product claim.']}


class RegistryValidation(unittest.TestCase):
    def invalid(self, transform, pattern):
        data={'repositories':[record()]}; transform(data)
        with self.assertRaisesRegex(ValueError,pattern): m.validate(data)

    def test_rejects_unknown_role_and_duplicate_repository(self):
        self.invalid(lambda d:d['repositories'][0]['roles'].append('Worker'),'known roles')
        self.invalid(lambda d:d['repositories'].append(record()),'duplicate repository')
        self.invalid(lambda d:d['repositories'][0].pop('visibility'),'explicit visibility')

    def test_rejects_duplicate_node_dangling_edge_and_missing_sources(self):
        self.invalid(lambda d:d['repositories'][0]['nodes'].append(copy.deepcopy(d['repositories'][0]['nodes'][0])),'duplicate node')
        self.invalid(lambda d:d['repositories'][0]['edges'][0].update(to='missing'),'endpoints')
        self.invalid(lambda d:d['repositories'][0].update(citations=[]),'citations required')

    def test_refuses_unsafe_output_name_source_path_and_url(self):
        self.invalid(lambda d:d['repositories'][0].update(repository='microsoft/..'),'owner/repository')
        self.invalid(lambda d:d['repositories'][0]['citations'][0].update(path='../secret'),'unsafe citation path')
        self.invalid(lambda d:d['repositories'][0]['start_cta'].update(url='javascript:alert(1)'),'HTTPS')
        self.invalid(lambda d:d['repositories'][0]['start_cta'].update(url='https://user:password@example.org'),'HTTPS')

    def test_containment_cycles_refused_but_dependency_cycles_allowed(self):
        r=record(); r['edges']=[{'from':'self','to':'engine','relation':'contains'},{'from':'engine','to':'self','relation':'contains'}]
        with self.assertRaisesRegex(ValueError,'containment cycle'):m.validate({'repositories':[r]})
        for e in r['edges']:e['relation']='uses'
        self.assertEqual(len(m.validate({'repositories':[r]})),1)

    def test_scope_overflow_fails_instead_of_silent_truncation(self):
        self.invalid(lambda d:d['repositories'][0].update(edges=d['repositories'][0]['edges']*7),'six relationship')


class RenderAndImpact(unittest.TestCase):
    def test_product_map_is_keyboard_scrollable_and_editorial_notes_are_in_details(self):
        page=m.product_page(record())
        self.assertIn('class="map-scroll" role="region" tabindex="0"',page)
        self.assertIn('Scroll the map or <a href="map.svg">open it full size</a>',page)
        self.assertIn('Outward roles: Agent Instance. Supports: Tool.',page)
        self.assertIn('.map-scroll img{min-width:960px',m.STYLE)
        split=page.index('<summary>Requirements, map sources, and implementation details</summary>')
        self.assertNotIn('Illustrative fixture; no product claim.',page[:split])
        self.assertIn('Illustrative fixture; no product claim.',page[split:])

    def test_private_publication_refused_and_explicit_local_paths_allowed(self):
        r=record();r['visibility']='private'
        with self.assertRaisesRegex(ValueError,'private records'):m.outputs(Path('/public'),[r])
        self.assertTrue(m.outputs(Path('/unused'),[r],Path('/owners'),True))
        self.assertTrue(m.outputs(Path('/private-preview'),[r],private_preview=True))

    def test_suppressed_external_link_keeps_identity_for_reverse_impact(self):
        r=record();r['nodes'][1]['link']=False
        self.assertNotIn('href="https://github.com/microsoft/engine"',m.map_svg(r))
        engine=record('microsoft/engine')
        self.assertEqual(m.affected([r,engine],['microsoft/engine']),['microsoft/engine','microsoft/example'])

    def test_deterministic_xml_escaping_and_links(self):
        r=record(); first=m.map_svg(r)
        self.assertEqual(first,m.map_svg(copy.deepcopy(r)))
        doc=ET.fromstring(first)
        self.assertIn('Engine <alternative>', ''.join(doc.itertext()))
        self.assertNotIn('<alternative>',first)
        self.assertIn('https://github.com/microsoft/engine',first)
        self.assertIn('uses · optional',first)
        self.assertIn('stroke-dasharray',first)

    def test_shared_nodes_appear_once_and_project_boundary_matches_ownership(self):
        r=record()
        r['nodes'] += [{'id':'worker','label':'Worker','kind':'internal'},
                       {'id':'adapter','label':'Adapter','kind':'internal'}]
        r['edges'] = [{'from':'self','to':'worker','relation':'contains'},
                      {'from':'self','to':'adapter','relation':'contains'},
                      {'from':'worker','to':'engine','relation':'uses'},
                      {'from':'adapter','to':'engine','relation':'uses'}]
        m.validate(r)
        doc=ET.fromstring(m.map_svg(r)); ns={'s':'http://www.w3.org/2000/svg'}
        rendered=doc.findall('.//*[@data-node-id]')
        self.assertCountEqual([n.attrib['data-node-id'] for n in rendered],[n['id'] for n in r['nodes']])
        boundaries=doc.findall('.//*[@data-project-boundary="true"]')
        self.assertEqual(len(boundaries),1)
        def box(element):
            return tuple(float(element.attrib[k]) for k in ('x','y','width','height'))
        bx,by,bw,bh=box(boundaries[0])
        for node in rendered:
            x,y,w,h=box(node.find('.//s:rect',ns))
            kind=next(n['kind'] for n in r['nodes'] if n['id']==node.attrib['data-node-id'])
            self.assertEqual(node.attrib['data-kind'],kind)
            if kind=='external':
                self.assertTrue(x>=bx+bw or x+w<=bx or y>=by+bh or y+h<=by)
            else:
                self.assertTrue(bx<=x and by<=y and x+w<=bx+bw and y+h<=by+bh)

    def test_all_directed_relationships_optional_scope_and_link_suppression_survive_rendering(self):
        r=record();r['nodes'][1]['link']=False
        r['nodes'] += [{'id':'worker','label':'Worker','kind':'internal'},
                       {'id':'peer','label':'Peer project','kind':'external','repository':'microsoft/peer'},
                       {'id':'contract','label':'Boundary contract','kind':'external'}]
        r['edges'] = [{'from':'self','to':'worker','relation':'contains'},
                      {'from':'worker','to':'engine','relation':'uses','optional':True},
                      {'from':'self','to':'peer','relation':'supports'},
                      {'from':'self','to':'contract','relation':'implements'},
                      {'from':'peer','to':'self','relation':'adapts'},
                      {'from':'peer','to':'engine','relation':'integrates'}]
        m.validate(r)
        doc=ET.fromstring(m.map_svg(r)); ns={'s':'http://www.w3.org/2000/svg'}
        edges=doc.findall('.//*[@data-edge-from]')
        self.assertCountEqual([(e.attrib['data-edge-from'],e.attrib['data-edge-to'],e.attrib['data-relation']) for e in edges],
                              [(e['from'],e['to'],e['relation']) for e in r['edges']])
        for edge in edges:
            path=edge.find('s:path',ns)
            self.assertEqual(path.attrib['marker-end'],'url(#arrow)')
            self.assertEqual('stroke-dasharray' not in path.attrib,edge.attrib['data-relation']=='contains')
        text=''.join(doc.itertext())
        self.assertIn('uses · optional',text)
        self.assertIn('Worker uses optionally Engine <alternative>.',text)
        hidden=doc.findall('.//*[@data-node-id="engine"]')
        self.assertEqual(len(hidden),1)
        self.assertIn('Engine <alternative>',''.join(hidden[0].itertext()))
        self.assertEqual(hidden[0].findall('.//s:a',ns),[])
        peer=doc.find('.//*[@data-node-id="peer"]')
        self.assertEqual(peer.find('s:a',ns).attrib['href'],'https://github.com/microsoft/peer')

    def test_second_row_containment_and_upward_use_do_not_cross_unrelated_parts(self):
        r=record();r['nodes']=r['nodes'][:1]
        r['nodes'] += [{'id':name,'label':name,'kind':'internal'} for name in ('alpha','beta','gamma','delta')]
        r['edges'] = [{'from':'self','to':name,'relation':'contains'} for name in ('alpha','beta','gamma','delta')]
        r['edges'].append({'from':'gamma','to':'alpha','relation':'uses'})
        m.validate(r)
        doc=ET.fromstring(m.map_svg(r));ns={'s':'http://www.w3.org/2000/svg'}
        bounds={}
        for node in doc.findall('.//*[@data-node-id]'):
            rect=node.find('.//s:rect',ns)
            bounds[node.attrib['data-node-id']]=tuple(float(rect.attrib[k]) for k in ('x','y','width','height'))
        for edge in doc.findall('.//*[@data-edge-from]'):
            path=edge.find('s:path',ns).attrib['d']
            self.assertRegex(path,r'^(?:[MHV]|-?\d+(?:\.\d+)?|\s)+$')
            tokens=re.findall(r'[MHV]|-?\d+(?:\.\d+)?',path)
            cursor=0;segments=[];point=None
            while cursor<len(tokens):
                command=tokens[cursor];cursor+=1
                if command=='M':
                    point=(float(tokens[cursor]),float(tokens[cursor+1]));cursor+=2
                else:
                    self.assertIn(command,('H','V'));self.assertIsNotNone(point)
                    value=float(tokens[cursor]);cursor+=1
                    next_point=(value,point[1]) if command=='H' else (point[0],value)
                    segments.append((point,next_point));point=next_point
            ends=(edge.attrib['data-edge-from'],edge.attrib['data-edge-to'])
            for node_id,(x,y,w,h) in bounds.items():
                if node_id in ends:continue
                for (x1,y1),(x2,y2) in segments:
                    # Four-pixel inset excludes border grazing and rounded corners.
                    left,right,top,bottom=x+4,x+w-4,y+4,y+h-4
                    crosses=(left<x1<right and max(min(y1,y2),top)<min(max(y1,y2),bottom)) if x1==x2 else (top<y1<bottom and max(min(x1,x2),left)<min(max(x1,x2),right))
                    with self.subTest(edge=ends,unrelated_node=node_id,segment=((x1,y1),(x2,y2))):
                        self.assertFalse(crosses,'Relationship route crosses an unrelated part')

    def test_reverse_impact_includes_transitive_consumers_and_terminates_on_cycle(self):
        a,b,c=record('microsoft/a'),record('microsoft/b'),record('microsoft/c')
        a['nodes'][1]['repository']='microsoft/b'; b['nodes'][1]['repository']='microsoft/c';c['nodes'][1]['repository']='microsoft/a'
        self.assertEqual(m.affected([a,b,c],['microsoft/c']),['microsoft/a','microsoft/b','microsoft/c'])
        with self.assertRaisesRegex(ValueError,'unknown repository'):m.affected([a],['microsoft/nope'])

    def test_provider_authored_relationship_finds_external_consumer(self):
        runtime=record('microsoft/runtime');instance=record('microsoft/instance')
        instance['nodes']=instance['nodes'][:1];instance['edges']=[]
        runtime['nodes'][1].update(id='consumer',repository='microsoft/instance')
        for relation,source,target in [('uses','consumer','self'),('contains','consumer','self'),('supports','self','consumer'),('adapts','self','consumer'),('integrates','self','consumer')]:
            with self.subTest(relation=relation):
                runtime['edges']=[{'from':source,'to':target,'relation':relation}]
                self.assertEqual(m.affected([runtime,instance],['microsoft/runtime']),['microsoft/instance','microsoft/runtime'])
        runtime['nodes'].append({'id':'inside','label':'Engine boundary','kind':'internal'})
        runtime['edges']=[{'from':'consumer','to':'inside','relation':'uses'}]
        self.assertEqual(m.affected([runtime,instance],['microsoft/runtime']),['microsoft/instance','microsoft/runtime'])

    def test_external_only_known_repository_can_start_impact_without_private_record(self):
        unified=record('microsoft/unified')
        unified['nodes'][1].update(repository='microsoft/private-client',link=False)
        unified['nodes'].append({'id':'coordinator','label':'Coordinator','kind':'internal'})
        self.assertEqual(m.affected([unified],['microsoft/private-client']),['microsoft/private-client','microsoft/unified'])
        with self.assertRaisesRegex(ValueError,'unknown repository'):m.affected([unified],['coordinator'])
        with self.assertRaisesRegex(ValueError,'unknown repository'):m.affected([unified],['microsoft/unseen'])

    def test_private_local_only_emits_no_central_site_even_with_same_root(self):
        root=Path('/example/staging')
        files=m.outputs(root,[record()],root,True)
        self.assertEqual(len(files),5)
        self.assertTrue(all('/example/docs/' in str(p) for p in files))
        self.assertFalse(any('/site/' in str(p) for p in files))
        page=files[root/'example/docs/product/index.html']
        self.assertIn('href="style.css"',page)
        self.assertIn('https://raw.githubusercontent.com/microsoft/amplifier-constellation/main/site/assets/art/building-blocks.png',page)

    def test_check_detects_drift_and_does_not_repair_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); registry=root/m.REGISTRY;registry.parent.mkdir(parents=True)
            registry.write_text(json.dumps({'repositories':[record()]}))
            with contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(m.main(['--root',tmp]),0)
                self.assertEqual(m.main(['--root',tmp,'--check']),0)
                obsolete=root/'site/projects/retired/map.svg';obsolete.parent.mkdir();obsolete.write_text('retained stale map')
                self.assertEqual(m.main(['--root',tmp,'--check']),1)
                self.assertEqual(obsolete.read_text(),'retained stale map');obsolete.unlink();obsolete.parent.rmdir()
                target=root/'site/projects/example/map.svg';target.write_text('changed')
                before=target.stat().st_mtime_ns
                self.assertEqual(m.main(['--root',tmp,'--check']),1)
                self.assertEqual(target.read_text(),'changed');self.assertEqual(target.stat().st_mtime_ns,before)
                self.assertEqual(m.main(['--root',tmp]),0)
                self.assertEqual(m.main(['--root',tmp,'--check']),0)

    def test_private_cli_uses_explicit_registry_without_public_outputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); registry=root/'owner.json';registry.write_text(json.dumps({'repositories':[record()]}))
            args=['--root',str(root/'public'),'--registry',str(registry),'--local-only','--staging',str(root/'owners')]
            with contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(m.main(args),0); self.assertEqual(m.main(args+['--check']),0)
            self.assertFalse((root/'public').exists())
            self.assertTrue((root/'owners/example/docs/product/index.html').is_file())

    def test_single_private_owner_record_can_be_checked_without_public_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);r=record();r['visibility']='private'
            owner=root/'owner';(owner/'docs').mkdir(parents=True)
            (owner/'docs/ecosystem-map.json').write_text(json.dumps(r,indent=2,ensure_ascii=False,sort_keys=True)+'\n')
            (owner/'docs/ecosystem-map.svg').write_text(m.map_svg(r))
            with contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(m.main(['--check-local',str(owner),'--root',str(root/'public')]),0)
                (owner/'docs/ecosystem-map.svg').write_text('stale')
                self.assertEqual(m.main(['--check-local',str(owner)]),1)
            self.assertFalse((root/'public').exists())


if __name__=='__main__':unittest.main()
