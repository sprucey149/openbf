#!/usr/bin/env python3
"""Inventory world attachments and prove unchanged shape/motion where possible.

This is a conservative companion to render_collision_check. Equality includes
every vertex, triangle, skin binding, skeleton, animation key and relevant RON
sidecar. It proves that the reference's residual is unchanged at every pose;
different geometry is UNMEASURED, never a pass from a few sampled poses. It does
not measure the reference's absolute animated residual. Water's baked plane is
also checked at every vertex. Reports retain names, counts and hashes, not art.
"""
import argparse
from collections import Counter
from functools import lru_cache
import hashlib
import json
import math
from pathlib import Path

from world_fingerprint import digest, glb_geometry, read_glb, read_ron, safe_child
from extract_lua import fnv1a

REQUIRED_CATEGORIES = ('doors_turrets', 'vehicles_hulls', 'soldier_feet',
                       'foliage', 'command_posts_pads', 'water', 'shadows',
                       'art_collision_presence')
HOLOGRAM_ICONS = ('alliance', 'imperial', 'republic', 'cis', 'turret')


def _stat(path):
    value = path.stat()
    return str(path.resolve()), value.st_mtime_ns, value.st_size


@lru_cache(maxsize=2048)
def _shape(path, modified, size):
    del modified, size
    doc, accessor = read_glb(Path(path))
    meshes, alpha_materials = [], set()
    for mesh in doc.get('meshes', []):
        parts = []
        for part in mesh.get('primitives', []):
            attrs = part.get('attributes', {})
            material = doc.get('materials', [])[part['material']] if 'material' in part else {}
            nonopaque = material.get('alphaMode', 'OPAQUE') != 'OPAQUE'
            if nonopaque and material.get('name'):
                alpha_materials.add(material['name'])
            if 'POSITION' not in attrs:
                raise ValueError(f'{path}: missing attachment positions')
            values = {key: accessor(value) for key, value in attrs.items()
                      if key == 'POSITION' or key.startswith(('JOINTS_', 'WEIGHTS_'))
                      or nonopaque and key.startswith(('TEXCOORD_', 'COLOR_'))}
            positions = values['POSITION']
            if any(len(p) != 3 or not all(math.isfinite(v) for v in p) for p in positions):
                raise ValueError(f'{path}: invalid attachment positions')
            indices = accessor(part['indices']) if 'indices' in part else [[i] for i in range(len(positions))]
            if any(len(i) != 1 or i[0] < 0 or i[0] >= len(positions) for i in indices):
                raise ValueError(f'{path}: invalid attachment indices')
            targets = [{key: accessor(value) for key, value in target.items()}
                       for target in part.get('targets', [])]
            parts.append({'attributes': values, 'indices': indices,
                          'mode': part.get('mode', 4), 'targets': targets,
                          'extras': part.get('extras'),
                          'alpha': {key: material.get(key) for key in ('name', 'alphaMode', 'alphaCutoff', 'doubleSided')}
                          if nonopaque else {'alphaMode': 'OPAQUE'},
                          'alpha_motion': material.get('extras') if nonopaque else None})
        meshes.append({'parts': parts, 'weights': mesh.get('weights'), 'extras': mesh.get('extras')})
    if not meshes and not doc.get('nodes') and not doc.get('animations'):
        raise ValueError(f'{path}: empty attachment mesh')
    nodes = [{k: v for k, v in node.items()
              if k in ('name', 'matrix', 'rotation', 'translation', 'scale', 'children', 'mesh', 'skin', 'weights')}
             for node in doc.get('nodes', [])]
    skins = [{key: accessor(value) if key == 'inverseBindMatrices' else value
              for key, value in skin.items()} for skin in doc.get('skins', [])]
    animations = [{'name': value.get('name'), 'extras': value.get('extras'),
                   'channels': value.get('channels', []),
                   'samplers': [{key: accessor(v) if key in ('input', 'output') else v
                                 for key, v in sampler.items()}
                                for sampler in value.get('samplers', [])]}
                  for value in doc.get('animations', [])]
    return (digest({'meshes': meshes, 'nodes': nodes, 'skins': skins,
                    'animations': animations, 'scenes': doc.get('scenes'), 'scene': doc.get('scene')}),
            tuple(sorted(alpha_materials)))


def shape(path):
    geometry, alpha_names = _shape(*_stat(path))
    folder = path.parent.parent
    return digest([geometry, texture_inventory(folder) if alpha_names else []])


def texture_inventory(folder):
    textures = []
    if folder:
        # Skin overrides and map/common fallbacks can change the selected alpha
        # sheet. Require the complete candidate texture inventory to be equal,
        # including both tiers, rather than guess a material's active override.
        folders = [folder / 'textures']
        if (folder / 'manifest.ron').is_file():
            folders.extend(folder.parent / f'common_{game}' / 'textures' for game in ('bf1', 'bf2'))
        for index, texture_folder in enumerate(folders):
            for candidate in sorted(texture_folder.glob('*.ktx2')):
                textures.append([index, candidate.name, _file_hash(*_stat(candidate))])
    return textures


@lru_cache(maxsize=16384)
def _file_hash(path, modified, size):
    del modified, size
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


@lru_cache(maxsize=2048)
def _ron(path, modified, size):
    del modified, size
    return read_ron(Path(path))


def ron(path):
    return _ron(*_stat(path))


def _row(category, name, reference, source, kind='exact_shape_and_motion'):
    equal = reference == source
    return {'category': category, 'name': name, 'status': 'pass' if equal else 'unmeasured',
            'evidence': kind if equal else 'different_shape_or_motion_requires_measurement',
            'reference_sha256': reference, 'source_sha256': source,
            'residual_excess_bound_m': 0.0 if equal else None}


def _files(reference, source, category, patterns):
    names = sorted({str(path.relative_to(root)) for root in (reference, source)
                    for pattern in patterns for path in root.glob(pattern) if path.is_file()})
    rows = []
    for name in names:
        values = []
        for root in (reference, source):
            path = root / name
            if not path.is_file():
                values.append(None)
            elif path.suffix == '.glb':
                values.append(shape(path))
            elif path.suffix == '.ron':
                values.append(digest(ron(path)))
            else:
                values.append(hashlib.sha256(path.read_bytes()).hexdigest())
        rows.append(_row(category, name, *values))
    return rows


def _worlds(doc):
    yield 'base', doc.get('instances', [])
    for i, layer in enumerate(doc.get('layers', [])):
        yield f'layer:{i}:{layer["name"]}', layer.get('instances', [])


def _geometry(doc, folder, name):
    matches = [row for row in doc.get('geometries', []) if row['name'].lower() == name.lower()]
    if len(matches) != 1:
        raise ValueError(f'{folder}: attachment geometry {name!r} resolves {len(matches)} times')
    row = matches[0]
    # Include every available LOD and the far extension, not only LOD0.
    models = {row['model']} | {model for _tier, model in row.get('lods', [])}
    if row.get('far'):
        models.add(row['far'])
    return digest([[model, shape(safe_child(folder, f'models/{model}.glb'))] for model in sorted(models)])


def _class_rows(root, key):
    folder = root / key
    doc = ron(folder / 'manifest.ron')
    common_folder = root / ('common_' + doc.get('source_game', 'bf1'))
    common = ron(common_folder / 'common.ron')
    classes = {}
    for origin, data in ((folder, doc), (common_folder, common)):
        for row in data.get('classes', []):
            classes.setdefault(row['name'].lower(), (row, origin, data))
    return folder, doc, classes


def _class_signature(item):
    if item is None:
        return None
    row, folder, doc = item
    geometry = row.get('geometry')
    model = _geometry(doc, folder, geometry) if geometry else None
    colls = [value for value in doc.get('collision', [])
             if value.get('geometry', '').lower() == (geometry or '').lower()]
    physical = []
    for coll in colls:
        physical.append([coll, shape(safe_child(folder, coll['glb'])) if coll.get('glb') else None])
    return digest({'class': row, 'render': model, 'collision': physical})


def _fallen_dependencies(root, key, doc):
    """Conservatively retain every authored wreck/pad and class motion input."""
    game = doc.get('source_game', 'bf1')
    paths = sorted((root / 'classes').glob(f'{game}_*.ron'))
    local = root / key / 'classes.ron'
    if local.is_file():
        paths.insert(0, local)
    common_folder = root / f'common_{game}'
    common = ron(common_folder / 'common.ron')
    geometry_names = {'com_prop_buildzone'}  # shared turret fall fallback
    definitions = []
    keys = {fnv1a('DestroyedGeometryName'), 0x97C931C5}
    for path in paths:
        values = ron(path)
        definitions.append([str(path.relative_to(root)), digest(values)])
        for value in values:
            geometry_names.update(str(text).strip() for key, text in value.get('props', []) if int(key) in keys and str(text).strip())
    models = []
    for name in sorted(geometry_names):
        value = None
        for folder, data in ((root / key, doc), (common_folder, common)):
            row = next((g for g in data.get('geometries', []) if g['name'].lower() == name.lower()), None)
            model = row['model'] if row else name
            if (folder / 'models' / (model + '.glb')).is_file():
                value = _geometry(data, folder, name) if row else shape(folder / 'models' / (model + '.glb'))
                break
        models.append([name, value])
    return digest({'class_inputs': definitions, 'wreck_pad_models': models})


def hologram_dependencies(root, key, game):
    """Direct HoloArt loads bypass manifest class/geometry and texture lists.

    The icon draw uses UVs and scroll even when the GLB material is OPAQUE.
    Hash the complete GLB plus the actual local/common file bindings for both
    texture tiers, rather than applying the ordinary world-material shortcut.
    """
    root = Path(root)
    common = root / f'common_{game}'
    rows = []
    for side in HOLOGRAM_ICONS:
        relative = f'models/com_icon_{side}.glb'
        source = root / key if side == 'turret' and (root / key / relative).is_file() else common
        path = source / relative
        if not path.is_file():
            rows.append({'name': side, 'sha256': None, 'missing': True, 'source': str(source.relative_to(root))})
            continue
        positions, indices = glb_geometry(path)
        if (not positions or not indices or any(len(point) != 3 or not all(math.isfinite(value) for value in point)
                                               for point in positions)):
            raise ValueError(f'invalid direct hologram geometry: {path}')
        doc, _accessor = read_glb(path)
        bindings = []
        for material in doc.get('materials', []):
            name = material.get('name', '').lower()
            if Path(name).name != name or '\\' in name:
                raise ValueError('unsafe hologram texture name')
            directory = source / 'textures' if (source / 'textures' / (name + '.ktx2')).is_file() else common / 'textures'
            base = directory / (name + '.ktx2')
            if not base.is_file():
                bindings.append([name, None])
                continue
            tier2, detail = directory / (name + '.tier2.ktx2'), directory / (name + '.dtl.ktx2')
            bindings.append([name, _file_hash(*_stat(base)),
                             _file_hash(*_stat(tier2)) if tier2.is_file() else _file_hash(*_stat(base)),
                             _file_hash(*_stat(detail)) if detail.is_file() else None])
        rows.append({'name': side, 'sha256': digest([_file_hash(*_stat(path)), bindings]),
                     'missing': False, 'source': str(source.relative_to(root))})
    return rows


def _water(folder, doc):
    water = doc.get('water')
    if water is None:
        return None, None
    positions, _indices = glb_geometry(safe_child(folder, water['glb']))
    if not positions:
        raise ValueError(f'{folder}: empty water plane')
    height = water['height']
    if not math.isfinite(height):
        raise ValueError(f'{folder}: nonfinite water height')
    worst = max(abs(position[1] - height) for position in positions)
    config = water.get('config') or {}
    return digest({'shape': shape(folder / water['glb']), 'height': height,
                   'wet': water.get('wet'), 'motion_and_shading': config}), worst


def attachment_map(reference: Path, source: Path, key: str):
    """Conservative, complete dependency inventory; differing paths fail closed."""
    reference, source = Path(reference), Path(source)
    objects, errors = [], []
    try:
        rf, rd, rc = _class_rows(reference, key)
        sf, sd, sc = _class_rows(source, key)
        game = rd.get('source_game', 'bf1')
        if sd.get('source_game', 'bf1') != game:
            raise ValueError('source game differs from reference')
        for name in sorted(set(rc) | set(sc)):
            items = [classes.get(name) for classes in (rc, sc)]
            rows = [item[0] if item else {} for item in items]
            if any(row.get('door') or row.get('turret_rig') for row in rows):
                objects.append(_row('doors_turrets', name, *map(_class_signature, items)))
            if any(row.get('base', '').lower() in ('commandpost', 'vehiclespawn') for row in rows):
                objects.append(_row('command_posts_pads', name, *map(_class_signature, items)))
        for a, b in zip(hologram_dependencies(reference, key, game), hologram_dependencies(source, key, game)):
            value = _row('command_posts_pads', f'hologram_icon:{a["name"]}', a['sha256'], b['sha256'])
            value.update(reference_missing=a['missing'], source_missing=b['missing'],
                         reference_lookup=a['source'], source_lookup=b['source'])
            objects.append(value)
        objects.append(_row('doors_turrets', 'all_authored_wrecks_pads_and_class_inputs',
                            _fallen_dependencies(reference, key, rd), _fallen_dependencies(source, key, sd)))
        for category, patterns in (
                ('vehicles_hulls', (f'side_{game}/vehicles/*/vehicles.ron',
                                    f'side_{game}/vehicles/*/models/*.glb',
                                    f'side_{game}/vehicles/*/collision/*.glb',
                                    f'side_{game}/vehicles/*/walkers/*.glb')),
                ('soldier_feet', (f'side_{game}/soldiers/*.glb', f'side_{game}/soldiers/*.ron')),
                ('shadows', (f'side_{game}/vehicles/*/shadows/*.ron', f'{key}/shadows/*.ron'))):
            rows = _files(reference, source, category, patterns)
            objects.extend(rows)
            if not rows and category in ('vehicles_hulls', 'soldier_feet'):
                errors.append(f'{category}: required side inventory missing')
        objects.append(_row('shadows', 'lighting_and_common_shadow_art',
                            digest([rd.get('lighting'), texture_inventory(reference / f'common_{game}')]),
                            digest([sd.get('lighting'), texture_inventory(source / f'common_{game}')])) )
        # Map-local vehicles are outside SIDE and must not escape the inventory.
        for name in sorted(set(rc) | set(sc)):
            items = [classes.get(name) for classes in (rc, sc)]
            if any(item and item[0].get('base', '').lower() in
                   ('hover', 'treaded', 'walker', 'flyer', 'carrier', 'commandhover', 'commandwalker', 'commandflyer')
                   for item in items):
                objects.append(_row('vehicles_hulls', f'map:{name}', *map(_class_signature, items)))
        foliage = []
        for folder, doc, classes in ((rf, rd, rc), (sf, sd, sc)):
            value = doc.get('foliage')
            value = value or {}
            terrain = doc.get('terrain_glb')
            meshes = [[mesh.get('name'), _geometry(doc, folder, mesh['name'])]
                      for mesh in value.get('meshes', [])
                      if mesh.get('kind') == 'Model' and mesh.get('name')]
            path_models = [[inst.get('class'), _class_signature(classes.get(inst.get('class', '').lower()))]
                           for inst in value.get('path_props', [])]
            foliage.append(digest({'manifest': value,
                                   'instances': hashlib.sha256(safe_child(folder, value['instances']).read_bytes()).hexdigest()
                                   if value.get('instances') else None,
                                   'models': meshes, 'path_models': path_models,
                                   'leaf_classes': doc.get('leaf', []),
                                   'wind': doc.get('wind'),
                                   'attachments': [[name, item[0].get('attach', [])] for name, item in sorted(classes.items())
                                                   if item[0].get('attach')],
                                   'alpha_candidates': texture_inventory(folder)
                                   if doc.get('leaf') or value else [],
                                   'terrain': shape(safe_child(folder, terrain)) if terrain else None,
                                   'terrain_query_size': doc.get('terrain_query_size')}))
        objects.append(_row('foliage', 'terrain_anchoring_and_models', *foliage))
        waters = [_water(folder, doc) for folder, doc in ((rf, rd), (sf, sd))]
        water = _row('water', 'all_plane_vertices_and_motion_flags', waters[0][0], waters[1][0])
        water['reference_plane_residual_m'] = waters[0][1]
        water['source_plane_residual_m'] = waters[1][1]
        if waters[0][1] is not None and waters[1][1] is not None:
            # RON decimal round trips can differ from the stored f32 by <1 um.
            if waters[1][1] > waters[0][1] + 1e-6:
                water['status'] = 'offender'
                water['evidence'] = 'water_vertex_above_reference_plane_residual'
        objects.append(water)
        # All placed art/physics presence remains the surface gate's responsibility.
        # Here record the exact inventory, including PS2's visual-only and invisible cases.
        for folder, doc, classes, label in ((rf, rd, rc, 'reference'), (sf, sd, sc, 'source')):
            counts = Counter()
            records = []
            for layer, instances in _worlds(doc):
                for index, inst in enumerate(instances):
                    item = classes.get(inst.get('class', '').lower())
                    row = item[0] if item else {}
                    geometry = row.get('geometry')
                    has_art = bool(geometry)
                    has_collision = bool(row.get('collision'))
                    presence = f'art:{int(has_art)}:collision_list:{int(has_collision)}'
                    counts[presence] += 1
                    records.append([layer, index, inst.get('name'), inst.get('class'), presence])
            objects.append({'category': 'art_collision_presence', 'name': label,
                            'status': 'pass', 'evidence': 'enumerated_for_placed_surface_gate',
                            'placements': sum(counts.values()), 'counts': dict(counts),
                            'inventory_sha256': digest(records),
                            'note': 'Collision lists alone do not determine physical presence; the surface gate resolves geometry and primitives.'})
    except (ValueError, KeyError, IndexError, TypeError, OSError) as error:
        errors.append(str(error))
    categories = {}
    for category in REQUIRED_CATEGORIES:
        rows = [row for row in objects if row['category'] == category]
        counts = dict(Counter(row['status'] for row in rows))
        categories[category] = {'items': len(rows), 'counts': counts,
                                'status': 'offender' if counts.get('offender') else
                                'unmeasured' if errors or counts.get('unmeasured') else 'pass'}
    status = ('offender' if any(row['status'] == 'offender' for row in objects) else
              'unmeasured' if errors or any(row['status'] == 'unmeasured' for row in objects) else 'pass')
    return {'method': 'exhaustive_attachment_dependency_identity_v1',
            'complete': status != 'unmeasured', 'status': status,
            'categories': categories, 'objects': objects, 'errors': errors,
            'baseline': 'Exact identity proves zero increase over PS2 at every pose. Absolute animated PS2 residuals and original-game captures are not measured here.'}


def attachment_problems(receipt):
    """Reject incomplete or structurally inconsistent attachment receipts."""
    errors = []
    if not isinstance(receipt, dict):
        return ['attachment receipt missing or not an object']
    if receipt.get('method') != 'exhaustive_attachment_dependency_identity_v1':
        errors.append('attachment method missing or unsupported')
    if not isinstance(receipt.get('complete'), bool) or receipt.get('status') not in ('pass', 'offender', 'unmeasured'):
        errors.append('attachment completeness or status invalid')
    categories = receipt.get('categories', {})
    objects = receipt.get('objects', [])
    if not isinstance(categories, dict) or not isinstance(objects, list):
        return errors + ['invalid attachment categories or objects']
    if set(categories) != set(REQUIRED_CATEGORIES):
        errors.append('attachment categories incomplete or unknown')
    for category in REQUIRED_CATEGORIES:
        rows = [row for row in objects if isinstance(row, dict) and row.get('category') == category]
        value = categories.get(category, {})
        if value.get('status') not in ('pass', 'offender', 'unmeasured') or value.get('items') != len(rows):
            errors.append(f'{category}: incomplete or inconsistent inventory')
        if value.get('counts') != dict(Counter(row.get('status') for row in rows)):
            errors.append(f'{category}: status counts differ from objects')
    if not isinstance(receipt.get('errors'), list):
        errors.append('attachment errors must be a list')
    for row in objects:
        if not isinstance(row, dict) or row.get('category') not in REQUIRED_CATEGORIES or row.get('status') not in ('pass', 'offender', 'unmeasured'):
            errors.append('invalid attachment object')
            continue
        if row.get('status') == 'unmeasured':
            if row.get('evidence') != 'different_shape_or_motion_requires_measurement' or row.get('residual_excess_bound_m') is not None:
                errors.append('unmeasured attachment claims a bound')
        elif row.get('status') == 'offender':
            if row.get('category') != 'water' or row.get('evidence') != 'water_vertex_above_reference_plane_residual':
                errors.append('unsupported measured attachment offender')
        elif row.get('evidence') == 'exact_shape_and_motion':
            if row.get('reference_sha256') != row.get('source_sha256') or row.get('residual_excess_bound_m') != 0:
                errors.append('attachment identity proof inconsistent')
        elif row.get('evidence') != 'enumerated_for_placed_surface_gate':
            errors.append('attachment evidence missing or unsupported')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', required=True, type=Path)
    parser.add_argument('--case', required=True, type=Path)
    parser.add_argument('--map', required=True)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    result = attachment_map(args.reference, args.case, args.map)
    args.out.write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')
    print(json.dumps({'map': args.map, 'status': result['status'], 'categories': result['categories'], 'errors': result['errors']}))
    return int(bool(attachment_problems(result)) or result['status'] != 'pass' or not result['complete'])


if __name__ == '__main__':
    raise SystemExit(main())
