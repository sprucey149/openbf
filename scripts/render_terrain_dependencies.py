#!/usr/bin/env python3
"""Actual terrain draw dependencies accompanying the geometric alignment proof.

Mirrors viewer::terrain_drawn, far_cull_from/lowres_range, Scene texture loading,
get_texture's map/common and _lowrez lookup, and fs_far_sheet's alpha test.
Both texture tiers and every supported cull/LOWR flag combination are checked.
RGB may differ. Nonuniform alpha needs native shape/UV/alpha identity because
implicit mip LOD depends on screen derivatives; affine UV equality alone is not
an all-view coverage proof after changing geometry.
"""
import hashlib
import json
import math
from pathlib import Path
import struct

from world_fingerprint import read_glb, read_ron, safe_child

KTX2 = b'\xabKTX 20\xbb\r\n\x1a\n'
SHEET_CUTOFF = struct.unpack('<f', struct.pack('<f', 0.1235))[0]
KINDS = ('policy', 'base_texture', 'far_texture', 'far_alpha', 'far_geometry')


def _finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def _digest(value):
    return hashlib.sha256(json.dumps(value, separators=(',', ':'), allow_nan=False).encode()).hexdigest()


def _record(kind, status, reason, reference=None, source=None, tier=None):
    return {'kind': kind, 'status': status, 'complete': status != 'unmeasured',
            'reason': reason, 'tier': tier, 'reference': reference, 'source': source}


def _policy(manifest, far_cull, lowres):
    game = manifest.get('source_game', 'bf1')
    if game not in ('bf1', 'bf2'):
        raise ValueError('unknown source_game in terrain draw policy')
    sky = manifest.get('sky') or {}
    lighting = manifest.get('lighting') or {}
    enable = sky.get('terrain_enable')
    if enable is not None and not _finite(enable):
        raise ValueError('nonfinite terrain enable policy')
    enabled = not (game == 'bf1' and enable == 0)
    near = None
    if far_cull:
        ranges = lighting.get('near_scene_range')
        if ranges is not None:
            if not isinstance(ranges, list) or len(ranges) != 3 or not all(map(_finite, ranges)):
                raise ValueError('invalid near scene range')
            if ranges[2] > 0:
                near = ranges[2]
    far_range = None
    if enabled and lowres and near is not None and manifest.get('lowres_glb'):
        if sky.get('low_res_terrain') is not None and lighting.get('far_scene_range') != 0:
            far = sky.get('low_res_max_distance')
            if far is not None and not _finite(far):
                raise ValueError('nonfinite low resolution max distance')
            far = 2000.0 if far is None else far
            if far > near:
                far_range = [near, far]
    return {'enabled': enabled, 'base': bool(enabled and manifest.get('terrain_glb')),
            'near_cull': near if enabled and manifest.get('terrain_glb') else None,
            'far_range': far_range, 'far_cull_flag': far_cull, 'lowres_flag': lowres}


def _ktx_descriptor(path):
    """Validate exactly the uncompressed RGBA8 mip representation uploaded.

    The runtime keeps a proper prefix of extra mips. Unsupported/short chains
    fail closed here instead of silently proving alpha from bytes not uploaded.
    """
    path = Path(path)
    size = path.stat().st_size
    with path.open('rb') as handle:
        header = handle.read(80)
        if len(header) != 80 or header[:12] != KTX2:
            raise ValueError(f'invalid KTX2 texture: {path.name}')
        fmt, unit, width, height, depth, layers, faces, count, compression = struct.unpack_from('<9I', header, 12)
        if (fmt not in (37, 43) or unit != 1 or not width or not height
                or depth not in (0, 1) or layers not in (0, 1) or faces != 1
                or compression != 0 or not 1 <= count <= max(width, height).bit_length()):
            raise ValueError(f'unsupported RGBA8 KTX2 representation: {path.name}')
        dfd_offset, dfd_length, kvd_offset, kvd_length, sgd_offset, sgd_length = struct.unpack_from('<4I2Q', header, 48)
        if (dfd_length < 4 or dfd_offset + dfd_length >= size or kvd_offset + kvd_length >= size
                or sgd_offset + sgd_length >= size):
            raise ValueError(f'invalid KTX2 metadata bounds: {path.name}')
        index = handle.read(count * 24)
        if len(index) != count * 24:
            raise ValueError(f'truncated KTX2 level index: {path.name}')
        spans = []
        for i in range(count):
            offset, length, unpacked = struct.unpack_from('<3Q', index, i * 24)
            expected = max(1, width >> i) * max(1, height >> i) * 4
            if length != expected or unpacked != expected or offset < 80 + count * 24 or offset + length > size:
                raise ValueError(f'KTX2 mip is not a complete RGBA8 level: {path.name}')
            if any(offset < end and start < offset + length for start, end in spans):
                raise ValueError(f'overlapping KTX2 mip payloads: {path.name}')
            spans.append((offset, offset + length))
    return {'width': width, 'height': height, 'levels': count, 'spans': spans}


def _alpha(path):
    descriptor = _ktx_descriptor(path)
    signature = hashlib.sha256()
    signature.update(struct.pack('<3I', descriptor['width'], descriptor['height'], descriptor['levels']))
    low, high = 255, 0
    with Path(path).open('rb') as handle:
        for i, (start, end) in enumerate(descriptor['spans']):
            handle.seek(start)
            rgba = handle.read(end - start)
            if len(rgba) != end - start:
                raise ValueError('texture changed during alpha proof')
            alpha = rgba[3::4]
            if not alpha:
                raise ValueError('empty RGBA8 alpha plane')
            signature.update(struct.pack('<3I', i, max(1, descriptor['width'] >> i), max(1, descriptor['height'] >> i)))
            signature.update(alpha)
            low, high = min(low, min(alpha)), max(high, max(alpha))
    return {'width': descriptor['width'], 'height': descriptor['height'], 'mips': descriptor['levels'],
            'min_alpha': low, 'max_alpha': high, 'alpha_sha256': signature.hexdigest()}


def _texture_path(folder, name, tier):
    if not isinstance(name, str) or not name or Path(name).name != name or name in ('.', '..'):
        raise ValueError('unsafe loaded texture name')
    base = folder / 'textures' / f'{name}.ktx2'
    override = folder / 'textures' / f'{name}.tier2.ktx2'
    selected = override if tier == 'pc' and override.is_file() else base
    _ktx_descriptor(selected)
    detail = folder / 'textures' / f'{name}.dtl.ktx2'
    if detail.is_file():
        _ktx_descriptor(detail)  # load_texture uploads this even though far alpha does not sample it.
    return selected


class LoadedTextures(dict):
    """Keep loadable-but-not-yet-draw-valid model facts with the registry."""
    def __init__(self):
        super().__init__()
        self.load_notes = []


def _inventory(root, key, manifest, tier):
    folder = root / key
    names = manifest.get('textures', [])
    if not isinstance(names, list) or any(not isinstance(name, str) for name in names):
        raise ValueError('invalid map loaded texture registry')
    loaded = LoadedTextures()
    for name in names:
        loaded[name.lower()] = {'path': _texture_path(folder, name, tier), 'owner': 'map', 'name': name}
    common = root / f"common_{manifest.get('source_game', 'bf1')}"
    common_error = None
    if (common / 'common.ron').is_file():
        try:
            data = read_ron(common / 'common.ron')
            # Any failed common asset aborts load_common before merge_common.
            if (not isinstance(data, dict) or not isinstance(data.get('models', []), list)
                    or not isinstance(data.get('textures', []), list)
                    or any(not isinstance(name, str) for name in data.get('models', []) + data.get('textures', []))):
                raise ValueError('invalid common loaded asset registry')
            for name in data.get('models', []):
                parts = _mesh_parts(safe_child(common, f'models/{name}.glb'), allow_empty=True, load_only=True)
                for ordinal, part in enumerate(parts):
                    if part.get('nonfinite_uv_rows'):
                        loaded.load_notes.append(dict(owner='common', model=name, primitive=ordinal,
                                                      nonfinite_uv_rows=part['nonfinite_uv_rows'],
                                                      file=f'models/{name}.glb'))
            for collision in data.get('collision', []):
                if collision.get('glb'):
                    _mesh_parts(safe_child(common, collision['glb']), allow_empty=True, load_only=True)
            additions = {}
            for name in data.get('textures', []):
                additions[name.lower()] = {'path': _texture_path(common, name, tier), 'owner': 'common', 'name': name}
            for name, row in additions.items():
                loaded.setdefault(name, row)
        except (ValueError, KeyError, TypeError, OSError, struct.error) as exc:
            common_error = str(exc)
    return loaded, common_error


def _lookup(loaded, name):
    if not isinstance(name, str):
        return None
    name = name.lower()
    return loaded.get(name) or (loaded.get(name[:-7]) if name.endswith('_lowrez') else None)


def _texture_record(row):
    if row is None:
        return {'loaded': False}
    descriptor = _ktx_descriptor(row['path'])
    return {'loaded': True, 'owner': row['owner'], 'name': row['name'], 'file': row['path'].name,
            'width': descriptor['width'], 'height': descriptor['height'], 'mips': descriptor['levels']}


def _mesh_parts(path, allow_empty=False, load_only=False):
    doc, accessor = read_glb(path)
    if doc.get('asset', {}).get('version') != '2.0':
        raise ValueError('invalid GLB asset version')
    if any('uri' in buffer for buffer in doc.get('buffers', [])) or doc.get('images'):
        raise ValueError('external/image GLB dependencies lack an actual loader certificate')
    materials = doc.get('materials', [])
    parts = []
    for mesh in doc.get('meshes', []):
        extras = mesh.get('extras') or {}
        if load_only and any(key in extras for key in ('render_grid_offset', 'render_grid_size')):
            raise ValueError('common model render-grid metadata needs its loader-specific proof')
        if 'contact_surface' in extras and type(extras['contact_surface']) is not bool:
            raise ValueError('contact_surface mesh metadata is not a bool')
        for primitive in mesh.get('primitives', []):
            if primitive.get('mode', 4) != 4:
                raise ValueError('unsupported terrain primitive mode')
            attributes = primitive['attributes']
            position_descriptor = doc['accessors'][attributes['POSITION']]
            if position_descriptor.get('componentType') != 5126 or position_descriptor.get('type') != 'VEC3':
                raise ValueError('terrain positions are not FLOAT32 VEC3')
            positions = accessor(attributes['POSITION'])
            indices = [row[0] for row in accessor(primitive['indices'])]
            if (not positions or not indices or len(indices) % 3 or any(i < 0 or i >= len(positions) for i in indices)
                    or any(len(p) != 3 or not all(map(_finite, p)) for p in positions)):
                raise ValueError('invalid indexed terrain primitive')
            uvs = accessor(attributes['TEXCOORD_0']) if 'TEXCOORD_0' in attributes else [[0.0, 0.0] for _ in positions]
            if len(uvs) != len(positions) or any(len(uv) != 2 for uv in uvs):
                raise ValueError('invalid terrain texture coordinate shape')
            nonfinite_uv = sum(not all(map(_finite, uv)) for uv in uvs)
            if nonfinite_uv and not load_only:
                raise ValueError('invalid terrain texture coordinates')
            alpha = [255] * len(positions)
            if 'COLOR_0' in attributes:
                index = attributes['COLOR_0']
                descriptor = doc['accessors'][index]
                colours = accessor(index)
                if len(colours) != len(positions) or descriptor['type'] not in ('VEC3', 'VEC4'):
                    raise ValueError('invalid terrain vertex colors')
                if descriptor['type'] == 'VEC4':
                    component = descriptor['componentType']
                    if component == 5121:
                        alpha = [row[3] for row in colours]
                    elif component == 5123:
                        alpha = [row[3] // 256 for row in colours]  # gltf Normalize<u8> for u16.
                    elif component == 5126:
                        if any(not _finite(row[3]) for row in colours):
                            raise ValueError('nonfinite vertex alpha')
                        alpha = [min(255, int(struct.unpack('<f', struct.pack('<f', max(0.0, row[3]) * 255.0))[0])) for row in colours]
                    else:
                        raise ValueError('unsupported terrain vertex alpha type')
            material = materials[primitive['material']] if 'material' in primitive else {}
            if not isinstance(material.get('name', ''), str):
                raise ValueError('invalid terrain material name')
            # Runtime ignores node transforms/baseColorFactor here and takes all
            # mesh primitives at identity. The signature uses exactly its data.
            uv_format = None
            if 'TEXCOORD_0' in attributes:
                descriptor = doc['accessors'][attributes['TEXCOORD_0']]
                uv_format = [descriptor['componentType'], descriptor.get('normalized', False)]
            if load_only:
                # assets::load_glb_joints copies finite and nonfinite UV values
                # alike; GPU upload checks lengths/indices, not UV finiteness.
                # A selected draw still takes the strict path above. Retain
                # this fact instead of inventing a failed common merge.
                parts.append({'name': material.get('name', ''), 'triangles': len(indices)//3,
                              'nonfinite_uv_rows': nonfinite_uv})
                continue
            shape = {'positions': positions, 'indices': indices, 'uv': uvs, 'uv_format': uv_format, 'alpha': alpha}
            parts.append({'name': material.get('name', ''), 'mode': material.get('alphaMode', 'OPAQUE'),
                          'triangles': len(indices) // 3, 'min_vertex_alpha': min(alpha[i] for i in indices),
                          'max_vertex_alpha': max(alpha[i] for i in indices),
                          'shape_sha256': _digest(shape), 'uv_sha256': _digest(uvs)})
    if not parts and not allow_empty:
        raise ValueError('terrain mesh has no indexed draw primitives')
    return parts


def _alpha_proof(reference_path, source_path, reference_mesh, source_mesh):
    ra, sa = _alpha(reference_path), _alpha(source_path)
    reference_min = min(p['min_vertex_alpha'] for p in reference_mesh)
    source_max = max(p['max_vertex_alpha'] for p in source_mesh)
    source_min = min(p['min_vertex_alpha'] for p in source_mesh)
    if sa['min_alpha'] * source_min / (255.0 * 255.0) >= SHEET_CUTOFF:
        return 'pass', 'every loaded mip and indexed vertex is above the far-sheet cutoff', ra, sa
    if (ra['min_alpha'] * reference_min / (255.0 * 255.0) >= SHEET_CUTOFF
            and sa['max_alpha'] * source_max / (255.0 * 255.0) < SHEET_CUTOFF):
        return 'offender', 'the reference far sheet is covered but every source far fragment is discarded', ra, sa
    same_alpha = ra['alpha_sha256'] == sa['alpha_sha256']
    same_shape = ([p['shape_sha256'] for p in reference_mesh] == [p['shape_sha256'] for p in source_mesh])
    if same_alpha and same_shape:
        return 'pass', 'native geometry, UVs, vertex alpha and all texture alpha mips match exactly', ra, sa
    return ('unmeasured', 'nonuniform far alpha lacks native geometry/UV/alpha identity; implicit mip LOD can change coverage',
            ra, sa)


def _direct_far_geometry(reference_folder, source_folder, rm, sm):
    # Extra donor far art has no native PS2 far counterpart. Prove it directly
    # against the canonical simulation grid/cutters, at the absolute 1 cm limit.
    from render_terrain_check import Grid, measure, MARGIN_M
    from world_fingerprint import glb_geometry
    vertices,_indices=glb_geometry(safe_child(reference_folder,rm['terrain_glb']))
    grid=Grid(vertices,rm.get('terrain_query_size'))
    measured,_cells=measure(safe_child(source_folder,sm['lowres_glb']),grid,rm.get('terrain_cutters',[]),True)
    if measured['height_worst_m']>MARGIN_M:
        status='offender'
    elif measured['vertical_triangles'] or not measured['coverage_complete']:
        status='unmeasured'
    else:
        status='pass'
    return _record('far_geometry',status,'extra donor far art is measured directly against canonical heights and cutters',
                   {'absolute_margin_m':MARGIN_M},
                   {key:measured.get(key) for key in ('triangles','height_worst_m','coverage_complete',
                                                     'coverage_upper_bound_m','worst_at','coverage_witness')})


def terrain_dependencies(reference_root, source_root, key, rm, sm):
    result = {'schema': 1, 'status': 'pass', 'complete': True, 'tiers': ['ps2', 'pc'], 'requirements': None, 'records': []}
    records = result['records']
    try:
        if not isinstance(key, str) or not key or Path(key).name != key or key in ('.', '..') or '\\' in key:
            raise ValueError('unsafe terrain dependency map key')
        reference_root, source_root = Path(reference_root), Path(source_root)
        reference_folder, source_folder = reference_root / key, source_root / key
        policies = []
        extra_far = False
        for far_cull, lowres in ((False, False), (False, True), (True, False), (True, True)):
            rp, sp = _policy(rm, far_cull, lowres), _policy(sm, far_cull, lowres)
            policies.append((rp, sp))
            comparable=dict(sp)
            additional=rp['far_range'] is None and sp['far_range'] is not None
            if additional:
                comparable['far_range']=None
                extra_far=True
            records.append(_record('policy', 'pass' if rp == comparable else 'offender',
                                   'actual terrain enable and range policy; extra source far art requires direct geometry and alpha proof' if additional
                                   else 'actual terrain enable and range policy', rp, sp))
        reference_enabled, source_enabled = policies[0][0]['base'], policies[0][1]['base']
        reference_far = policies[-1][0]['far_range'] is not None
        source_far = policies[-1][1]['far_range'] is not None
        reference_base = _mesh_parts(safe_child(reference_folder, rm['terrain_glb'])) if reference_enabled else []
        source_base = _mesh_parts(safe_child(source_folder, sm['terrain_glb'])) if source_enabled else []
        reference_far_mesh = _mesh_parts(safe_child(reference_folder, rm['lowres_glb'])) if reference_far else []
        source_far_mesh = _mesh_parts(safe_child(source_folder, sm['lowres_glb'])) if source_far else []
        result['requirements'] = {'base_primitives':len(source_base),
                                  'base_records_per_tier':max(1,len(source_base)) if reference_enabled or source_enabled else 0,
                                  'far_records_per_tier':int(reference_far or source_far),
                                  'direct_far_required':extra_far}
        if extra_far:
            records.append(_direct_far_geometry(reference_folder,source_folder,rm,sm))
        for tier in ('ps2', 'pc'):
            ri, reference_common_error = _inventory(reference_root, key, rm, tier)
            si, source_common_error = _inventory(source_root, key, sm, tier)
            if reference_enabled or source_enabled:
                # Terrain base emitters use one opaque material. A changed
                # non-opaque pass needs its own alpha proof, never an exemption.
                if any(p['mode'] != 'OPAQUE' for p in source_base):
                    records.append(_record('policy', 'unmeasured', 'source terrain base uses a non-opaque render pass', tier=tier))
                reference_loaded = any(_lookup(ri, p['name']) for p in reference_base)
                if not source_base:
                    records.append(_record('base_texture','offender' if reference_enabled and reference_loaded else 'pass',
                                           'source has no base primitive; draw policy records the coverage difference',
                                           {'loaded':reference_loaded},{'loaded':False,'no_mesh':True},tier))
                for part in source_base:
                    source_texture = _lookup(si, part['name'])
                    state = 'offender' if reference_loaded and source_texture is None else 'pass'
                    records.append(_record('base_texture', state,
                                           'loaded base texture' if source_texture else 'base chunks are skipped: texture is not in the loaded-name registry',
                                           {'loaded': reference_loaded, 'common_error': reference_common_error},
                                           {**_texture_record(source_texture), 'material': part['name'],
                                            'triangles': part['triangles'], 'common_error': source_common_error}, tier))
            if reference_far or source_far:
                reference_name = (rm.get('sky') or {}).get('low_res_terrain', '')
                source_name = (sm.get('sky') or {}).get('low_res_terrain', '')
                rt, st = _lookup(ri, reference_name), _lookup(si, source_name)
                records.append(_record('far_texture', 'offender' if source_far and not st and (extra_far or reference_far and rt) else 'pass',
                                       'far-sheet binding uses the sky texture name, independently of the GLB material',
                                       _texture_record(rt), _texture_record(st), tier))
                if reference_far and rt and source_far and st:
                    status, reason, ra, sa = _alpha_proof(rt['path'], st['path'], reference_far_mesh, source_far_mesh)
                    records.append(_record('far_alpha', status, reason, ra, sa, tier))
                elif source_far and st:
                    alpha=_alpha(st['path'])
                    minimum=min(p['min_vertex_alpha'] for p in source_far_mesh)
                    maximum=max(p['max_vertex_alpha'] for p in source_far_mesh)
                    if alpha['min_alpha']*minimum/(255.0*255.0)>=SHEET_CUTOFF:
                        state='pass'
                    elif alpha['max_alpha']*maximum/(255.0*255.0)<SHEET_CUTOFF:
                        state='offender'
                    else:
                        state='unmeasured'
                    records.append(_record('far_alpha',state,'far art without a native binding counterpart must cover every loaded mip',
                                           None,alpha,tier))
                else:
                    records.append(_record('far_alpha','pass','no source far fragments are drawn; the binding and policy records state why',
                                           {'far_active':reference_far,'bound':rt is not None},
                                           {'far_active':source_far,'bound':st is not None},tier))
    except (ValueError, KeyError, IndexError, TypeError, OSError, OverflowError, struct.error) as exc:
        records.append(_record('policy', 'unmeasured', str(exc)))
    if any(row['status'] == 'offender' for row in records):
        result['status'] = 'offender'
    elif any(row['status'] == 'unmeasured' for row in records):
        result['status'] = 'unmeasured'
    result['complete'] = all(row['complete'] for row in records)
    return result


def dependency_problems(receipt):
    if not isinstance(receipt, dict) or receipt.get('schema') != 1 or type(receipt.get('complete')) is not bool:
        return ['invalid terrain dependency receipt schema']
    errors = []
    if receipt.get('status') not in ('pass', 'offender', 'unmeasured'):
        errors.append('invalid terrain dependency status')
    if receipt.get('tiers') != ['ps2', 'pc']:
        errors.append('terrain dependencies omit a texture tier')
    records = receipt.get('records')
    if not isinstance(records, list) or not records:
        return errors + ['missing terrain dependency records']
    policies = []
    for row in records:
        if isinstance(row, dict) and row.get('kind') == 'policy' and isinstance(row.get('reference'), dict):
            for side in ('reference','source'):
                policy=row.get(side)
                if not isinstance(policy,dict) or any(type(policy.get(field))is not bool for field in ('enabled','base','far_cull_flag','lowres_flag')):
                    errors.append('invalid typed terrain draw policy')
                    continue
                near=policy.get('near_cull');far=policy.get('far_range')
                if (near is not None and (not _finite(near) or near<=0)
                        or far is not None and (not isinstance(far,list) or len(far)!=2 or not all(map(_finite,far)) or not 0<far[0]<far[1])
                        or policy['base'] and not policy['enabled']
                        or not policy['far_cull_flag'] and (near is not None or far is not None)
                        or not policy['lowres_flag'] and far is not None):
                    errors.append('invalid terrain draw range policy')
            flags = (row['reference'].get('far_cull_flag'), row['reference'].get('lowres_flag'))
            if all(type(flag) is bool for flag in flags):
                policies.append(flags)
            else:
                errors.append('invalid terrain cull/LOWR flag')
        if (not isinstance(row, dict) or row.get('kind') not in KINDS
                or row.get('status') not in ('pass', 'offender', 'unmeasured')
                or type(row.get('complete')) is not bool or not isinstance(row.get('reason'), str)
                or row.get('tier') not in (None, 'ps2', 'pc')):
            errors.append('invalid terrain dependency record')
        elif row['complete'] != (row['status'] != 'unmeasured'):
            errors.append('terrain dependency record completion disagrees with status')
    if receipt.get('status') == 'pass' and sorted(policies) != [(False, False), (False, True), (True, False), (True, True)]:
        errors.append('passing terrain dependencies omit a cull/LOWR variant')
    if receipt.get('status') == 'pass' and (not receipt['complete'] or any(row.get('status') != 'pass' for row in records if isinstance(row, dict))):
        errors.append('passing terrain dependencies contain an unproven record')
    if receipt['complete'] != all(row.get('complete') is True for row in records if isinstance(row, dict)):
        errors.append('terrain dependency completion disagrees with records')
    if receipt.get('status')=='pass':
        required=receipt.get('requirements')
        if (not isinstance(required,dict) or type(required.get('base_primitives'))is not int
                or required['base_primitives']<0 or type(required.get('base_records_per_tier'))is not int
                or required['base_records_per_tier']<0 or type(required.get('far_records_per_tier'))is not int
                or required['far_records_per_tier']not in (0,1) or type(required.get('direct_far_required'))is not bool):
            errors.append('missing terrain dependency requirements')
            return errors
        policy_rows=[row for row in records if isinstance(row,dict) and row.get('kind')=='policy'
                     and isinstance(row.get('reference'),dict) and isinstance(row.get('source'),dict)]
        base_active=any(row[side].get('base')is True for row in policy_rows for side in ('reference','source'))
        source_base=any(row['source'].get('base')is True for row in policy_rows)
        far_active=any(row[side].get('far_range')is not None for row in policy_rows for side in ('reference','source'))
        extra_far=any(row['reference'].get('far_range')is None and row['source'].get('far_range')is not None for row in policy_rows)
        if (source_base and required['base_primitives']<1 or not source_base and required['base_primitives']!=0
                or required['base_records_per_tier']!=(max(1,required['base_primitives']) if base_active else 0)
                or required['far_records_per_tier']!=int(far_active) or required['direct_far_required']!=extra_far):
            errors.append('terrain dependency requirements disagree with draw policy')
        for tier in ('ps2','pc'):
            for kind,count in (('base_texture',required['base_records_per_tier']),
                               ('far_texture',required['far_records_per_tier']),('far_alpha',required['far_records_per_tier'])):
                if sum(row.get('kind')==kind and row.get('tier')==tier for row in records if isinstance(row,dict))!=count:
                    errors.append(f'missing or duplicate {tier} {kind} proof')
        direct=[row for row in records if isinstance(row,dict) and row.get('kind')=='far_geometry']
        if len(direct)!=int(extra_far):
            errors.append('missing or duplicate direct far geometry proof')
        for row in direct:
            source=row.get('source') or {};reference=row.get('reference') or {}
            if (reference.get('absolute_margin_m')!=0.01 or source.get('coverage_complete')is not True
                    or any(not _finite(source.get(field)) or not 0<=source[field]<=0.01
                           for field in ('height_worst_m','coverage_upper_bound_m'))):
                errors.append('invalid direct far geometry bound')
    return errors
