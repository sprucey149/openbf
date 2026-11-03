"""World model surfaces actually submitted by viewer::scene.

A file on disk is not a loaded texture/model. Map registries win, common merges
only after its complete load succeeds, and unbound material names follow the
renderer census policy. RGB never enters a coverage signature. Nonuniform alpha
requires identical indexed geometry, UVs, effective vertex alpha and every mip.
"""
from functools import lru_cache
from pathlib import Path
import math
import struct

from render_terrain_dependencies import _alpha, _inventory, _lookup, _mesh_parts
from world_fingerprint import digest, glb_geometry, read_glb, read_ron, safe_child

DANGLING = ('yav_prop_ivy', 'cor1_bldg_flowers', 'noicon', 'tat_propcz1_parts_head')
TIERS = ('ps2', 'pc')


def draw_inventory(scene, tier):
    cache = getattr(scene, 'draw_inventory_cache', None)
    if cache is None:
        scene.draw_inventory_cache = cache = {}
    if tier not in cache:
        loaded, common_error = _inventory(scene.assets, scene.folder.name, scene.doc, tier)
        if common_error is not None:
            raise ValueError('complete common load/draw validity is uncertified: ' + common_error)
        models = {}
        common = scene.assets / ('common_' + scene.doc['source_game'])
        if (common / 'common.ron').is_file():
            doc = read_ron(common / 'common.ron')
            for name in doc.get('models', []):
                models.setdefault(name, safe_child(common, 'models/' + name + '.glb'))
        names = scene.doc.get('models', [])
        if not isinstance(names, list) or any(not isinstance(name, str) for name in names):
            raise ValueError('invalid loaded model registry')
        for name in names:
            path = safe_child(scene.folder, 'models/' + name + '.glb')
            parts = _mesh_parts(path, allow_empty=True, load_only=True)
            for ordinal, part in enumerate(parts):
                if part.get('nonfinite_uv_rows'):
                    loaded.load_notes.append(dict(owner='map',model=name,primitive=ordinal,
                                                  nonfinite_uv_rows=part['nonfinite_uv_rows'],file='models/'+name+'.glb'))
            models[name] = path
        folded = {}
        for name, path in models.items():
            if name.lower() in folded and folded[name.lower()] != path:
                raise ValueError('ambiguous case-insensitive loaded model registry: ' + name)
            folded[name.lower()] = path
        notes = []
        for note in loaded.load_notes:
            note = dict(note)
            owner = scene.folder if note['owner']=='map' else common
            expected = (owner/note['file']).resolve()
            consumers = []
            for identity, instance in getattr(scene,'instances',{}).items():
                try:
                    geometry,_model = scene.resolve(instance)
                    if geometry is not None and any(path is not None and path.resolve()==expected
                        for paths in scene.render_states(geometry,instance).values() for path in paths):
                        consumers.append(list(identity))
                except (ValueError,KeyError,TypeError,OSError):
                    continue
            note['world_instance_ids'] = consumers
            note['draw_validity'] = 'strict selected-model checks and the required attachment receipt remain mandatory'
            notes.append(note)
        if not hasattr(scene,'model_load_notes'):scene.model_load_notes={}
        scene.model_load_notes[tier] = notes
        cache[tier] = loaded, folded
    return cache[tier]


def route(name, binding):
    if binding is not None:
        return 'textured'
    if not name:
        return 'disc_untextured_skip'
    return 'dangling_skip' if name.lower() in DANGLING else 'lookup_failed_gray'


@lru_cache(maxsize=4096)
def alpha_at(path, modified, size):
    del modified, size
    return _alpha(Path(path))


def _parts(path):
    parts = _mesh_parts(path, allow_empty=True)
    doc, accessor = read_glb(path)
    originals = [part for mesh in doc.get('meshes', []) for part in mesh.get('primitives', [])]
    for part, original in zip(parts, originals):
        material = doc.get('materials', [])[original['material']] if 'material' in original else {}
        extras = material.get('extras') or {}
        # assets::mesh::light_path uses these strings; unknown/mistyped is Lit.
        light = extras.get('light')
        if light not in ('prelit', 'prelit_add', 'unlit', 'unlit_add', 'lit', 'glow', 'unscaled', 'vertex_lit'):
            light = 'lit'
        scroll = extras.get('uv_scroll', [0., 0.])
        if scroll is None:
            scroll = [0., 0.]
        if not isinstance(scroll, list) or len(scroll) != 2 or any(type(v) not in (int,float) or not math.isfinite(v) for v in scroll):
            raise ValueError('invalid material UV scroll')
        if part['mode'] not in ('OPAQUE', 'MASK', 'BLEND'):
            raise ValueError('invalid model alpha mode')
        # shape_sha256 includes positions, indices, UVs and effective u8 alpha.
        part.update(light=light, scroll=scroll)
    return parts


def _coverage(part, alpha, route_name):
    if route_name.endswith('_skip'):
        return 'absent'
    if route_name == 'lookup_failed_gray' or part['mode'] == 'OPAQUE':
        return 'full'
    lo, hi = alpha['min_alpha'], alpha['max_alpha']
    va, vb = part['min_vertex_alpha'], part['max_vertex_alpha']
    if part['light'] == 'glow':
        if part['mode'] == 'BLEND':
            return 'full'  # shade_unfogged returns coverage 1 for Glow.
        minimum = min(255, (((lo * 129) // 256) * va) // 128)
        maximum = min(255, (((hi * 129) // 256) * vb) // 128)
        return 'full' if minimum >= 96 else 'absent' if maximum < 96 else 'partial'
    if part['mode'] == 'MASK':
        # Shader threshold is fixed .753, not glTF alphaCutoff. Bounds safely
        # avoid the rounding boundary; equality near it remains partial.
        return ('full' if lo * va > .75301 * 255 * 255 else
                'absent' if hi * vb < .75299 * 255 * 255 else 'partial')
    return 'full' if lo == va == 255 else 'absent' if hi == 0 or vb == 0 else 'partial'


def model_descriptor(path, scene):
    path = Path(path)
    stat = path.stat()
    key = str(path), stat.st_mtime_ns, stat.st_size
    cache = getattr(scene, 'model_dependency_cache', None)
    if cache is None:
        scene.model_dependency_cache = cache = {}
    if key in cache:
        return cache[key]
    parts = _parts(path)
    if not parts:
        raise ValueError('model has no draw primitives')
    tiers = ('ps2',) if scene.reference_art else TIERS
    records, signatures = [], []
    covered, opaque = True, True
    for tier in tiers:
        textures, models = draw_inventory(scene, tier)
        registered = models.get(path.stem.lower())
        if registered is None or registered.resolve() != path.resolve():
            raise ValueError('model file is not the model loaded by its manifest registry: ' + path.name)
        partial = []
        for ordinal, part in enumerate(parts):
            binding = _lookup(textures, part['name'])
            selected = route(part['name'], binding)
            alpha = None
            if binding is not None:
                tex = binding['path']; info = tex.stat()
                alpha = alpha_at(str(tex), info.st_mtime_ns, info.st_size)
            coverage = _coverage(part, alpha, selected)
            covered &= coverage == 'full'
            effective_opaque = coverage == 'full' and (selected == 'lookup_failed_gray' or part['mode'] == 'OPAQUE')
            opaque &= effective_opaque and not part['light'].endswith('_add')
            record = dict(tier=tier, primitive=ordinal, material=part['name'], route=selected,
                          coverage=coverage, mode=part['mode'], light=part['light'],
                          min_vertex_alpha=part['min_vertex_alpha'], max_vertex_alpha=part['max_vertex_alpha'],
                          shape_sha256=part['shape_sha256'], scroll=part['scroll'],
                          texture=None if binding is None else dict(owner=binding['owner'], name=binding['name'], **alpha))
            records.append(record)
            if coverage != 'full':
                partial.append(dict(primitive=ordinal, coverage=coverage, shape=part['shape_sha256'],
                                    mode=part['mode'], light=part['light'], scroll=part['scroll'],
                                    alpha=None if alpha is None else alpha['alpha_sha256']))
        # A partial/absent mesh may not use its unsubmitted triangles as a
        # correspondence surface for a differently partitioned opaque model.
        signatures.append(None if not partial else digest(dict(partial=partial, geometry_sha256=digest(glb_geometry(path)))))
    unique = sorted(set(signatures), key=lambda value: value or '')
    result = dict(schema=1, complete=True, status='pass', tiers=list(tiers), records=records,
                  model_registered=True, all_surfaces_covered=bool(covered), borrow_opaque=bool(opaque),
                  indexed_geometry_sha256=digest(glb_geometry(path)),
                  visibility_signatures=unique, opacity_signature=None if unique == [None] else digest(unique),
                  model_load_notes={tier:scene.model_load_notes[tier] for tier in tiers})
    cache[key] = result
    return result


def model_dependencies(reference, source, paths_a, paths_b):
    a, b = [model_descriptor(path, reference) for path in paths_a], [model_descriptor(path, source) for path in paths_b]
    # Fully covered geometry is certified by the separate continuous metric;
    # only coverage-changing dependencies require exact shape/UV signatures.
    sig_a = [r['opacity_signature'] for r in a if r['opacity_signature'] is not None]
    sig_b = [r['opacity_signature'] for r in b if r['opacity_signature'] is not None]
    equal = sig_a == sig_b
    return dict(schema=1, complete=equal, status='pass' if equal else 'unmeasured', reference=a, source=b,
                reason='same effective drawn coverage' if equal else 'different effective alpha/skip coverage needs visible-surface measurement')


def dependency_problems(receipt):
    if not isinstance(receipt, dict) or receipt.get('schema') != 1 or receipt.get('status') not in ('pass','unmeasured'):
        return ['missing/invalid world draw dependency receipt']
    problems=[]
    if type(receipt.get('complete')) is not bool or (receipt['status']=='pass') != receipt['complete']:
        problems.append('inconsistent world draw dependency completion')
    sides=[]
    for name, tiers in (('reference',['ps2']), ('source',list(TIERS))):
        rows=receipt.get(name)
        if not isinstance(rows,list) or not rows:
            problems.append('missing world draw dependency model side'); continue
        signatures=[]
        for row in rows:
            if (not isinstance(row,dict) or row.get('schema')!=1 or row.get('model_registered') is not True
                    or row.get('status')!='pass' or row.get('complete') is not True or row.get('tiers')!=tiers
                    or type(row.get('all_surfaces_covered')) is not bool or type(row.get('borrow_opaque')) is not bool):
                problems.append('invalid world draw model receipt');continue
            records=row.get('records')
            if not isinstance(records,list) or not records:
                problems.append('missing world draw primitives');continue
            by_tier={tier:[] for tier in tiers}
            for record in records:
                if not isinstance(record,dict) or record.get('tier') not in by_tier or type(record.get('primitive')) is not int:
                    problems.append('invalid world draw primitive');continue
                by_tier[record['tier']].append(record['primitive'])
                if record.get('route') not in ('textured','disc_untextured_skip','dangling_skip','lookup_failed_gray') or record.get('coverage') not in ('full','partial','absent'):
                    problems.append('invalid world draw route/coverage')
                if record.get('route')=='textured' and not isinstance(record.get('texture'),dict):
                    problems.append('missing bound texture alpha evidence')
            if (not by_tier[tiers[0]] or any(values != list(range(len(by_tier[tiers[0]]))) for values in by_tier.values())):
                problems.append('incomplete world draw primitive/tier coverage')
            if row['all_surfaces_covered'] != all(r.get('coverage')=='full' for r in records):
                problems.append('forged full world draw coverage')
            per_tier=[]
            for tier in tiers:
                partial=[]
                for r in (v for v in records if v.get('tier')==tier):
                    alpha=r.get('texture')
                    valid_alpha=(isinstance(alpha,dict) and all(type(alpha.get(k)) is int for k in
                                 ('width','height','mips','min_alpha','max_alpha')) and alpha['width']>0 and alpha['height']>0
                                 and alpha['mips']>0 and 0<=alpha['min_alpha']<=alpha['max_alpha']<=255
                                 and isinstance(alpha.get('alpha_sha256'),str) and len(alpha['alpha_sha256'])==64)
                    if r.get('route')=='textured' and not valid_alpha:
                        problems.append('invalid all-mip world texture alpha proof');continue
                    if (r.get('mode') not in ('OPAQUE','MASK','BLEND') or not isinstance(r.get('light'),str)
                            or type(r.get('min_vertex_alpha')) is not int or type(r.get('max_vertex_alpha')) is not int
                            or not 0<=r['min_vertex_alpha']<=r['max_vertex_alpha']<=255):
                        problems.append('invalid effective world shader inputs');continue
                    part=dict(mode=r['mode'],light=r['light'],min_vertex_alpha=r['min_vertex_alpha'],max_vertex_alpha=r['max_vertex_alpha'])
                    if _coverage(part,alpha,r['route']) != r.get('coverage'):
                        problems.append('forged world shader coverage')
                    if r.get('coverage')!='full':
                        partial.append(dict(primitive=r['primitive'],coverage=r['coverage'],shape=r.get('shape_sha256'),
                                            mode=r['mode'],light=r['light'],scroll=r.get('scroll'),
                                            alpha=None if alpha is None else alpha.get('alpha_sha256')))
                per_tier.append(None if not partial else digest(dict(partial=partial,geometry_sha256=row.get('indexed_geometry_sha256'))))
            unique=sorted(set(per_tier),key=lambda v:v or '')
            expected=None if unique==[None] else digest(unique)
            if row.get('visibility_signatures')!=unique or row.get('opacity_signature')!=expected:
                problems.append('world draw signature does not match its evidence')
            if expected is not None:
                signatures.append(expected)
        sides.append(signatures)
    if receipt.get('status')=='pass' and (len(sides)!=2 or sides[0]!=sides[1]):
        problems.append('world draw coverage signatures differ')
    return problems
