#!/usr/bin/env python3
"""Whole-scene drawn-surface against PS2-collision sweep, ranked by excess over PS2's own residual.

    render_alignment_sweep.py --reference PS2_ASSETS --case xbox=ASSETS [--map KEY ...] --out report.json

render_alignment_probe.py measures each placed model against its own collision. That flags many points that a
neighbouring object or the terrain covers, so this sweep composes the scene before ranking. Each source/map gets:

- placed objects: active soldier-layer collision samples (forward) and drawn-art samples (reverse) of every rigid
  placement, first model-local, then, for every local excess above the margin, against the whole drawn scene of base
  plus the same mode layer (opaque faces, faces that alpha might cover, and the terrain draw mesh) and the whole
  scene's soldier collision plus the PS2 terrain. Scene excess is the source residual minus PS2's residual at the same
  point, so PS2's own art-to-collision residual is the baseline everywhere;
- terrain: the existing exhaustive terrain facet check (render_terrain_check.terrain_map) against the PS2 query grid;
- vehicles: every side vehicle's top LOD against its PS2 soldier-layer collision, model-local (vehicles move; there is
  no settled world point).

Every witness says whether it lies inside the authored playable boundary and how far it is above or below the PS2
terrain. Being inside the boundary is not a proof of a walking route, and opaque scene distance is not a camera
visibility proof: the ranking guides fixes, it accepts nothing. Reports hold coordinates and numbers, never geometry.
"""
import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

from render_alignment_probe import LayerContact, MARGIN_M
from render_collision_check import (REQUIRED_MAPS, Scene, Surface, bounds, box_distance2, dot, placed,
                                    primitive_distance, primitive_samples, require_rigid, sub, surface,
                                    surface_samples)
from render_contact_check import draw_surfaces, layer_bodies
from world_fingerprint import glb_geometry, read_ron, safe_child

METHOD = 'scene_composed_ordinary_float_samples'
# The gate's maps: small enough to bake alone (`REBAKE_OUT=... scripts/rebake.sh common maps` with
# OPENBF_BF1_MAPS/OPENBF_BF2_MAPS narrowed), and each holds repaired donor art that a regression would move.
GATE_MAPS = ('bf1_bes2', 'bf1_hot1', 'bf2_pol1', 'bf1_rhn2', 'bf2_fel1')
GATE_CEILING_M = .25
# Near-model limits retain the smaller repairs even when a map's distant PS2
# model has a larger native residual. These apply before the general exceptions.
GATE_NEAR_LIMITS = {
    ('bf1_rhn2', 'rhn_bldg_ruin_13'): .01,
    ('bf1_rhn2', 'rhn_bldg_wall_facade_pillar_h3a'): .01,
    ('bf1_rhn2', 'rhn_bldg_look_out2'): .08,
    ('bf2_fel1', 'fel1_bldg_bladder'): .15,
}
# (source, map, geometry) still above the ceiling: (its measured worst in metres, the reason). A listed row is never a
# pass for anything else, and it fails again when it grows more than GATE_SLACK_M past its measured worst.
GATE_KNOWN = {
    ('xbox', 'bf1_bes2', 'bes2_bldg_mall'): (0.477, 'donor draws faces off its collision (Xbox) and its PC LOWD leaves the floor undrawn'),
    ('xbox', 'bf1_bes2', 'com_item_healthrecharge'): (0.615, 'donor health-droid LOWD stand-in is larger than PS2 LOWD'),
    ('xbox', 'bf1_hot1', 'com_item_healthrecharge'): (0.615, 'donor health-droid LOWD stand-in is larger than PS2 LOWD'),
    ('xbox', 'bf1_hot1', 'hoth_bldg_corridor_natural'): (0.709, 'donor rock faces off collision; partial alpha'),
    ('xbox', 'bf1_hot1', 'hoth_bldg_hangar_entry'): (9.116, 'donor LOWD extends 19 m past PS2 LOWD into the hill'),
    ('xbox', 'bf1_hot1', 'hoth_bldg_large_hangars'): (16.109, 'donor has no LOWD; its primary model is drawn where PS2 draws a 50-face LOWD'),
    ('xbox', 'bf1_hot1', 'hoth_bldg_tunnel_exit'): (1.727, 'donor LOWD faces off collision'),
    ('xbox', 'bf1_hot1', 'hoth_prop_trench_short'): (0.550, 'donor trench faces off collision'),
    ('xbox', 'bf1_hot1', 'hoth_prop_trench_turrets'): (0.266, 'donor trench faces off collision'),
    ('pc1', 'bf1_bes2', 'bes2_bldg_chamber_exit'): (3.225, 'PC LOWD is a 1008-face cluster spanning neighbours'),
    ('pc1', 'bf1_bes2', 'bes2_bldg_chamber_shaft'): (0.909, 'PC shaft draws faces off collision'),
    ('pc1', 'bf1_bes2', 'bes2_bldg_courtyard_overlook'): (0.399, 'PC overlook leaves collision undrawn'),
    ('pc1', 'bf1_bes2', 'bes2_bldg_mall'): (3.836, 'donor draws faces off its collision (Xbox) and its PC LOWD leaves the floor undrawn'),
    ('pc1', 'bf1_bes2', 'bes2_bldg_sidewalk'): (0.374, 'PC sidewalk draws faces off collision'),
    ('pc1', 'bf1_bes2', 'bes2_bldg_skyway_ramp'): (0.298, 'PC ramp draws faces off collision'),
    ('pc1', 'bf1_bes2', 'bes2_bldg_square_bot2'): (1.030, 'PC square draws a face 1 m off collision'),
    ('pc1', 'bf1_bes2', 'com_item_healthrecharge'): (0.599, 'donor health-droid LOWD stand-in is larger than PS2 LOWD'),
    ('pc1', 'bf1_hot1', 'com_item_healthrecharge'): (0.599, 'donor health-droid LOWD stand-in is larger than PS2 LOWD'),
    ('pc1', 'bf1_hot1', 'hoth_bldg_corridor_natural'): (0.709, 'donor rock faces off collision; partial alpha'),
    ('pc1', 'bf1_hot1', 'hoth_bldg_hangar_entry'): (9.115, 'donor LOWD extends 19 m past PS2 LOWD into the hill'),
    ('pc1', 'bf1_hot1', 'hoth_bldg_large_hangars'): (16.109, 'donor has no LOWD; its primary model is drawn where PS2 draws a 50-face LOWD'),
    ('pc1', 'bf1_hot1', 'hoth_bldg_tunnel_exit'): (1.726, 'donor LOWD faces off collision'),
    ('pc1', 'bf1_hot1', 'hoth_prop_trench_short'): (0.550, 'donor trench faces off collision'),
    ('pc1', 'bf1_hot1', 'hoth_prop_trench_turrets'): (0.266, 'donor trench faces off collision'),
    # Outstanding drawing and distant-model differences on the added maps.
    # These are bounded regressions, not whole-model acceptance.
    ('xbox', 'bf1_rhn2', 'com_item_healthrecharge'): (.615, 'donor health-droid distant stand-in differs from PS2 LOWD'),
    ('pc1', 'bf1_rhn2', 'com_item_healthrecharge'): (.600, 'donor health-droid distant stand-in differs from PS2 LOWD'),
    ('xbox', 'bf1_rhn2', 'rhn_bldg_look_out'): (.889, 'remaining near wall and upper debris mismatch'),
    ('pc1', 'bf1_rhn2', 'rhn_bldg_look_out'): (.890, 'remaining near wall and upper debris mismatch'),
    ('xbox', 'bf1_rhn2', 'rhn_bldg_ruin_14'): (.384, 'remaining donor wall shape mismatch'),
    ('pc1', 'bf1_rhn2', 'rhn_bldg_ruin_14'): (.384, 'remaining donor wall shape mismatch'),
    ('xbox', 'bf1_rhn2', 'rhn_bldg_ruin_rotunda'): (1.037, 'near wall mismatch and full donor art compared with PS2 LOWD'),
    ('pc1', 'bf1_rhn2', 'rhn_bldg_ruin_rotunda'): (1.037, 'near wall mismatch and full donor art compared with PS2 LOWD'),
    ('xbox', 'bf1_rhn2', 'rhn_bldg_ruin_rotunda_upper'): (1.260, 'full donor art compared with PS2 LOWD'),
    ('pc1', 'bf1_rhn2', 'rhn_bldg_ruin_rotunda_upper'): (1.260, 'full donor art compared with PS2 LOWD'),
    ('xbox', 'bf1_rhn2', 'rhn_bldg_statue_alcove'): (.461, 'full donor art compared with PS2 LOWD'),
    ('pc1', 'bf1_rhn2', 'rhn_bldg_statue_alcove'): (.461, 'full donor art compared with PS2 LOWD'),
    ('xbox', 'bf1_rhn2', 'rhn_bldg_look_out2'): (.400, 'full donor art compared with PS2 LOWD; near limit remains .080 m'),
    ('pc1', 'bf1_rhn2', 'rhn_bldg_look_out2'): (.606, 'full donor art compared with PS2 LOWD; near limit remains .080 m'),
    ('xbox', 'bf1_rhn2', 'rhn_bldg_wall_facade_pillar_l'): (.251, 'remaining distant-model drawing residual'),
    ('pc1', 'bf1_rhn2', 'rhn_bldg_wall_facade_pillar_l'): (.251, 'remaining distant-model drawing residual'),
    ('xbox', 'bf2_fel1', 'fel1_bldg_segmented'): (.692, 'unrepaired donor drawing residual'),
    ('pc1', 'bf2_fel1', 'fel1_bldg_segmented'): (.692, 'unrepaired donor drawing residual'),
    ('xbox', 'bf2_fel1', 'fel1_bldg_red_bush'): (.600, 'unrepaired donor drawing residual'),
    ('pc1', 'bf2_fel1', 'fel1_bldg_red_bush'): (.600, 'unrepaired donor drawing residual'),
    ('xbox', 'bf2_fel1', 'fel1_bldg_mtn_accessorytall'): (2.182, 'unrepaired donor drawing residual'),
    ('pc1', 'bf2_fel1', 'fel1_bldg_mtn_accessorytall'): (2.182, 'unrepaired donor drawing residual'),
}
GATE_SLACK_M = .05
GATE_MAP_SLACK_M = {'bf1_rhn2': .005, 'bf2_fel1': .005}
# (source, map, placement name) the gate maps cannot measure, with the reason. Any other unmeasured placement fails.
GATE_UNMEASURED = {
    ('pc1', 'bf1_bes2', 'building_tower20'): 'PC placement record lacks PS2 prop 4137097213 ("W")',
    ('pc1', 'bf1_bes2', 'chamber entry'): 'PC placement record lacks PS2 prop 4137097213',
}
# Scene work is done for the largest local witnesses of each placement and state only.
SCENE_WITNESSES = 24


def local_point(point, inst):
    delta = sub(point, inst['translation'])
    return tuple(dot(column, delta) for column in inst['rotation'])


class Terrain:
    """The drawn terrain mesh of one bake, and the PS2 terrain height under any point."""
    def __init__(self, scene):
        name = scene.doc.get('terrain_glb')
        self.surface = surface(str(safe_child(scene.folder, name))) if name else None
        self.cutters = bool(scene.doc.get('terrain_cutters'))

    def distance(self, point):
        return self.surface.distance(point) if self.surface is not None else math.inf

    def heights(self, x, z):
        """Heights of every drawn terrain face straight above or below (x, z); empty off the terrain."""
        if self.surface is None:
            return []
        lo, hi = self.surface.bounds[1] - 1, self.surface.bounds[4] + 1
        found = []
        for face in self.surface.nearby_faces((x, lo, z, x, hi, z)):
            a, b, c = self.surface.triangles[face]
            det = (b[0]-a[0])*(c[2]-a[2]) - (c[0]-a[0])*(b[2]-a[2])
            if det == 0:
                continue
            u = ((x-a[0])*(c[2]-a[2]) - (c[0]-a[0])*(z-a[2])) / det
            v = ((b[0]-a[0])*(z-a[2]) - (x-a[0])*(b[2]-a[2])) / det
            if u < -1e-9 or v < -1e-9 or u + v > 1 + 1e-9:
                continue
            found.append(a[1] + u*(b[1]-a[1]) + v*(c[1]-a[1]))
        return found


class Providers:
    """Placed drawn surfaces of one scene, with world boxes for pruning."""
    def __init__(self, scene, terrain):
        self.items, self.unknown, cache = [], [], {}
        self.terrain = terrain
        for identity, inst in scene.instances.items():
            try:
                require_rigid(inst)
                geometry, path = scene.resolve(inst)
                if path is None:
                    continue
                paths = scene.render_states(geometry, inst)['model']
                if None in paths:
                    raise ValueError('missing configured draw model')
                if paths not in cache:
                    cache[paths] = draw_surfaces(scene, paths)
                potential, covered, _draws = cache[paths]
                if potential is None:
                    continue
                lo, hi = potential.bounds[:3], potential.bounds[3:]
                box = bounds([placed(tuple((lo, hi)[(k >> i) & 1][i] for i in range(3)), inst) for k in range(8)])
                self.items.append(dict(identity=identity, inst=inst, potential=potential, covered=covered,
                                       box=box, geometry=geometry))
            except (ValueError, TypeError, KeyError, OSError, IndexError) as error:
                self.unknown.append(dict(identity=list(identity), name=inst.get('name'), reason=str(error)))

    def nearest(self, point, layer, kind, exclude=None):
        """Distance to the nearest drawn surface of `kind` ('covered' or 'potential') and what supplied it.
        `exclude` leaves one placement out, so a caller can supply that placement's other LOD itself."""
        best, which = self.terrain.distance(point), 'terrain'
        ranked = sorted((box_distance2(point, item['box']), n) for n, item in enumerate(self.items)
                        if item['identity'][0] in (0, layer) and item[kind] is not None
                        and item['identity'] != exclude)
        for lower, n in ranked:
            if lower > best * best:
                break
            item = self.items[n]
            d = item[kind].distance(local_point(point, item['inst']))
            if d < best:
                best, which = d, item['geometry']
        return best, which


class Collisions:
    """Placed soldier-layer collision of the PS2 scene plus the PS2 terrain."""
    def __init__(self, scene, terrain):
        self.items, cache = [], {}
        self.terrain = terrain
        for identity, inst in scene.instances.items():
            try:
                require_rigid(inst)
                geometry, _model = scene.resolve(inst)
                key = (geometry, json.dumps(scene.collision_policy(inst), sort_keys=True))
                if key not in cache:
                    contact = LayerContact(scene, inst, 'soldier')
                    cache[key] = contact if contact.mesh is not None or contact.primitives else None
                contact = cache[key]
                if contact is None:
                    continue
                points = [p for p in contact.points(8)]
                if contact.mesh is not None:
                    points.extend(contact.mesh.vertices)
                extent = [p for prim in contact.primitives for p in primitive_samples(prim)]
                local = bounds(points + extent)
                lo, hi = local[:3], local[3:]
                box = bounds([placed(tuple((lo, hi)[(k >> i) & 1][i] for i in range(3)), inst) for k in range(8)])
                self.items.append(dict(identity=identity, inst=inst, contact=contact, box=box, geometry=geometry))
            except (ValueError, TypeError, KeyError, OSError, IndexError):
                continue

    def nearest(self, point, layer):
        best, which = self.terrain.distance(point), 'terrain'
        ranked = sorted((box_distance2(point, item['box']), n) for n, item in enumerate(self.items)
                        if item['identity'][0] in (0, layer))
        for lower, n in ranked:
            # Primitive boxes come from samples, so allow a little slack before pruning.
            if lower > (best + .5) ** 2:
                break
            item = self.items[n]
            d = item['contact'].distance(local_point(point, item['inst']))
            if d < best:
                best, which = d, item['geometry']
        return best, which


def local_witnesses(contact, reference_art, source_art, source_covered, count):
    """Model-local forward and reverse residuals at every sample (render_alignment_probe's method), measured on
    every drawn face of both bakes; `opaque_extra_m` repeats the forward value against the source's opaque faces."""
    forward, reverse = [], []
    for point in contact.points(count):
        baseline, candidate = reference_art.distance(point), source_art.distance(point)
        opaque = source_covered.distance(point) if source_covered is not None else math.inf
        forward.append(dict(direction='forward', point=point, ps2_m=baseline, source_m=candidate,
                            extra_m=candidate-baseline, opaque_extra_m=opaque-baseline))
    for point in surface_samples(source_art, count):
        _distance, closest = reference_art.closest(point)
        baseline, candidate = contact.distance(closest), contact.distance(point)
        reverse.append(dict(direction='reverse', point=point, ps2_point=closest, ps2_m=baseline, source_m=candidate,
                            extra_m=candidate-baseline,
                            opaque=source_covered is not None and source_covered.distance(point) < 1e-4))
    return forward, reverse


def place(row, terrain, scene):
    x, y, z = row['world']
    heights = terrain.heights(x, z)
    row['in_playable_boundary'] = scene.in_play(row['world'])
    # Campaign layers are not loaded by Instant Action.
    row['instant_action'] = 'campaign' not in str(row.get('layer', '')).lower()
    # Signed height above the nearest PS2 terrain face in this column; below every face usually means buried.
    row['above_ps2_terrain_m'] = min((y - h for h in heights), key=abs) if heights else None
    row['below_all_ps2_terrain'] = bool(heights) and all(y < h - .05 for h in heights) and not terrain.cutters
    return row


def sweep_objects(reference, source, samples, progress=None):
    ref_terrain, src_terrain = Terrain(reference), Terrain(source)
    ref_draw, src_draw = Providers(reference, ref_terrain), Providers(source, src_terrain)
    collisions = Collisions(reference, ref_terrain)
    rows, local_cache, unknown, art_only, coverage = [], {}, [], [], []
    for identity, inst in reference.instances.items():
        try:
            other = source.instances.get(identity)
            if other is None:
                raise ValueError('placement missing from source')
            require_rigid(inst)
            if inst != other:
                raise ValueError('source placement differs from PS2')
            geometry, _model = reference.resolve(inst)
            source_geometry, _source_model = source.resolve(other)
            if geometry is None:
                continue
            if (reference.collision_policy(inst) != source.collision_policy(other)
                    or reference.collision_identity(geometry) != source.collision_identity(source_geometry)):
                raise ValueError('source collision or layer policy differs from PS2')
            states, source_states = reference.render_states(geometry, inst), source.render_states(source_geometry, other)
            for state in ('model', 'lowd'):
                a, b = states.get(state, states['model']), source_states.get(state, source_states['model'])
                if None in a + b:
                    raise ValueError('missing configured draw model')
                if state == 'lowd' and a == states['model'] and b == source_states['model']:
                    continue
                key = (a, b, geometry, json.dumps(reference.collision_policy(inst), sort_keys=True))
                if key not in local_cache:
                    contact = LayerContact(reference, inst, 'soldier')
                    ref_potential, ref_covered, ref_records = draw_surfaces(reference, a)
                    potential, covered, records = draw_surfaces(source, b)
                    partial = any('partial' in r['coverage'] for r in ref_records + records)
                    same = [glb_geometry(Path(p)) for p in a] == [glb_geometry(Path(p)) for p in b]
                    opaque = [len(x.triangles) if x is not None else 0 for x in (ref_covered, covered)]
                    if same:
                        # Same drawn shape: no geometric gap. Opacity can still differ (another texture's alpha).
                        if opaque[0] != opaque[1]:
                            coverage.append(dict(identity=list(identity), geometry=geometry, state=state,
                                                 ps2_opaque_triangles=opaque[0], source_opaque_triangles=opaque[1],
                                                 triangles=len(potential.triangles) if potential else 0))
                        local_cache[key] = dict(witnesses=[], partial=partial)
                    elif contact.mesh is None and not contact.primitives:
                        # Drawn with no soldier collision on PS2 either: compare the art to PS2's art.
                        far = []
                        if potential is not None and ref_potential is not None:
                            for point in surface_samples(potential, samples):
                                d = ref_potential.distance(point)
                                far.append(dict(direction='art_only', point=point, ps2_m=0.0, source_m=d, extra_m=d))
                        local_cache[key] = dict(witnesses=far, partial=partial)
                    elif ref_potential is None or potential is None:
                        local_cache[key] = dict(witnesses=None, partial=partial, reason='no drawn surface')
                    else:
                        forward, reverse = local_witnesses(contact, ref_potential, potential, covered, samples)
                        local_cache[key] = dict(witnesses=forward + reverse, partial=partial)
                local = local_cache[key]
                if local['witnesses'] is None:
                    unknown.append(dict(identity=list(identity), geometry=geometry, state=state, reason=local['reason']))
                    continue
                flagged = sorted((w for w in local['witnesses'] if w['extra_m'] > MARGIN_M),
                                 key=lambda w: -w['extra_m'])[:SCENE_WITNESSES]
                for w in flagged:
                    world = placed(w['point'], inst)
                    row = dict(identity=list(identity), layer=reference.layer_names[identity[0]], name=inst.get('name'),
                               geometry=geometry, state=state, direction=w['direction'], local_point=list(w['point']),
                               world=list(world), local_ps2_m=w['ps2_m'], local_source_m=w['source_m'],
                               local_extra_m=w['extra_m'], partial_alpha=local['partial'],
                               local_opaque_extra_m=w.get('opaque_extra_m'))
                    if w['direction'] == 'art_only':
                        row['scene_extra_m'] = w['extra_m']
                        row['note'] = 'drawn without soldier collision on every source; distance to PS2 art'
                        art_only.append(place(row, ref_terrain, reference)); continue
                    if state != 'model' and w['direction'] == 'forward':
                        # This placement draws its LOWD; its neighbours are composed at their primary state.
                        ps2_pot, ps2_by = ref_draw.nearest(world, identity[0], 'potential', exclude=identity)
                        src_pot, src_by = src_draw.nearest(world, identity[0], 'potential', exclude=identity)
                        if w['ps2_m'] < ps2_pot:
                            ps2_pot, ps2_by = w['ps2_m'], geometry
                        if w['source_m'] < src_pot:
                            src_pot, src_by = w['source_m'], geometry
                        row.update(ps2_scene_m=ps2_pot, source_scene_m=src_pot, ps2_scene_by=ps2_by,
                                   source_scene_by=src_by, scene_extra_m=src_pot - ps2_pot)
                    elif w['direction'] == 'forward':
                        ps2_pot, ps2_by = ref_draw.nearest(world, identity[0], 'potential')
                        src_pot, src_by = src_draw.nearest(world, identity[0], 'potential')
                        ps2_cov, _ = ref_draw.nearest(world, identity[0], 'covered')
                        src_cov, _ = src_draw.nearest(world, identity[0], 'covered')
                        row.update(ps2_scene_m=ps2_pot, source_scene_m=src_pot, ps2_scene_by=ps2_by,
                                   source_scene_by=src_by, scene_extra_m=src_pot - ps2_pot,
                                   scene_extra_opaque_m=src_cov - ps2_cov)
                    else:
                        row['opaque_art'] = w['opaque']
                        ps2_point = placed(w['ps2_point'], inst)
                        src_d, src_by = collisions.nearest(world, identity[0])
                        ps2_d, ps2_by = collisions.nearest(ps2_point, identity[0])
                        row.update(ps2_scene_m=ps2_d, source_scene_m=src_d, ps2_scene_by=ps2_by,
                                   source_scene_by=src_by, scene_extra_m=src_d - ps2_d)
                    rows.append(place(row, ref_terrain, reference))
        except (ValueError, KeyError, TypeError, OSError, IndexError) as error:
            unknown.append(dict(identity=list(identity), name=inst.get('name'), reason=str(error)))
        if progress:
            progress()
    extra = sorted(set(source.instances) - set(reference.instances))
    return dict(witnesses=rows, art_only=art_only, coverage=coverage, unknown=unknown, extra_source_placements=[list(i) for i in extra],
                provider_unknown=dict(reference=ref_draw.unknown, source=src_draw.unknown),
                placements=len(reference.instances))


def sweep_terrain(reference_root, source_root, key):
    from render_terrain_check import terrain_map
    receipt = terrain_map(reference_root, source_root, key)
    surfaces = [dict(name=s['name'], status=s['status'], excess_m=s.get('excess_m'), height_worst_m=s.get('height_worst_m'),
                     worst_at=s.get('worst_at'), identical=s.get('reference_bytes_identical'),
                     coverage_upper_bound_m=s.get('coverage_upper_bound_m'))
                for s in receipt.get('surfaces', [])]
    return dict(status=receipt['status'], worst_excess_m=receipt['worst_excess_m'], worst_at=receipt['worst_at'],
                error=receipt.get('error'), surfaces=surfaces)


def vehicle_contact(folder, entry, cls):
    bodies = ([('collisionmesh', 0)] if entry.get('glb') else []) + [
        (str(p['name']).lower(), int(p.get('mask', 0))) for p in entry.get('prims', [])]
    active = layer_bodies(bodies, cls.get('collision', []), 'soldier')
    offset = int(bool(entry.get('glb')))
    mesh = surface(str(safe_child(folder, entry['glb']))) if offset and active[0] else None
    prims = [p for i, p in enumerate(entry.get('prims', [])) if active[offset + i]
             and p['kind'] in ('box', 'sphere', 'cylinder')]
    return mesh, prims


def sweep_vehicles(reference_root, source_root, samples):
    rows = []
    for manifest in sorted(reference_root.glob('side_bf*/vehicles/*/vehicles.ron')):
        relative = manifest.relative_to(reference_root)
        other = source_root / relative
        if not other.exists():
            rows.append(dict(manifest=str(relative), status='unknown', reason='source vehicle manifest missing')); continue
        doc, source_doc = read_ron(manifest), read_ron(other)
        geometries = {g['name'].lower(): g for g in doc.get('geometries', [])}
        collisions = {str(c['geometry']).lower(): c for c in doc.get('collision', [])}
        source_collisions = {str(c['geometry']).lower(): c for c in source_doc.get('collision', [])}
        seen = set()
        for row in doc.get('vehicles', []):
            cls = row['class']
            geometry = str(cls.get('geometry') or '').lower()
            if not geometry or geometry not in geometries:
                continue
            g = geometries[geometry]
            model = next((name for tier, name in g.get('lods', []) if tier.lower() == 'lod0'), g['model'])
            key = (model, json.dumps(cls.get('collision', [])))
            if key in seen:
                continue
            seen.add(key)
            record = dict(manifest=str(relative), vehicle=cls['name'], model=model)
            try:
                entry = collisions.get(geometry)
                if entry is None:
                    record.update(status='no_collision'); rows.append(record); continue
                if json.dumps(entry, sort_keys=True) != json.dumps(source_collisions.get(geometry), sort_keys=True):
                    raise ValueError('source vehicle collision differs from PS2')
                if entry.get('glb') and (glb_geometry(safe_child(manifest.parent, entry['glb'])) !=
                                         glb_geometry(safe_child(other.parent, entry['glb']))):
                    raise ValueError('source vehicle collision mesh differs from PS2')
                ref_model, src_model = manifest.parent / 'models' / f'{model}.glb', other.parent / 'models' / f'{model}.glb'
                if ref_model.read_bytes() == src_model.read_bytes():
                    record.update(status='identical'); rows.append(record); continue
                mesh, prims = vehicle_contact(manifest.parent, entry, cls)
                if mesh is None and not prims:
                    record.update(status='no_active_contact'); rows.append(record); continue
                ref_art, src_art = surface(str(ref_model)), surface(str(src_model))

                def contact_distance(point):
                    values = [mesh.distance(point)] if mesh is not None else []
                    values.extend(primitive_distance(point, p) for p in prims)
                    return min(values)
                points = (surface_samples(mesh, samples) if mesh is not None else []) + [
                    p for prim in prims for p in primitive_samples(prim)]
                best = None
                for point in points:
                    extra = src_art.distance(point) - ref_art.distance(point)
                    if best is None or extra > best['extra_m']:
                        best = dict(direction='forward', local_point=list(point), extra_m=extra,
                                    ps2_m=ref_art.distance(point), source_m=src_art.distance(point))
                for point in surface_samples(src_art, samples):
                    _d, closest = ref_art.closest(point)
                    extra = contact_distance(point) - contact_distance(closest)
                    if extra > best['extra_m']:
                        best = dict(direction='reverse', local_point=list(point), extra_m=extra,
                                    ps2_m=contact_distance(closest), source_m=contact_distance(point))
                record.update(status='offender' if best['extra_m'] > MARGIN_M else 'sample_clear', worst=best)
            except (ValueError, KeyError, TypeError, OSError, IndexError) as error:
                record.update(status='unknown', reason=str(error))
            rows.append(record)
    return rows


def gate_problems(report, ceiling, known, unmeasured=None, slack=GATE_SLACK_M):
    """Scene witnesses inside the playable boundary, not buried and above the ceiling: unlisted, or more than `slack`
    past the measured worst `known` records for them; and every unmeasured placement not listed in `unmeasured`."""
    problems, unmeasured = [], unmeasured or {}
    for row in report['maps']:
        objects = row.get('objects', {})
        if objects.get('error'):
            problems.append(f"{row['source']} {row['map']} objects unmeasured: {objects['error']}")
            continue
        worst, near_worst = {}, {}
        for w in objects.get('witnesses', []):
            limit = GATE_NEAR_LIMITS.get((row['map'], w['geometry'])) if w.get('state') == 'model' else None
            if (limit is not None and w['scene_extra_m'] > limit and w['in_playable_boundary']
                    and not w.get('below_all_ps2_terrain') and w.get('instant_action', True)):
                near_worst[w['geometry']] = max(near_worst.get(w['geometry'], 0), w['scene_extra_m'])
            if (w['scene_extra_m'] > ceiling and w['in_playable_boundary'] and not w.get('below_all_ps2_terrain')
                    and w.get('instant_action', True)):
                if w['scene_extra_m'] > worst.get(w['geometry'], (0, None))[0]:
                    worst[w['geometry']] = (w['scene_extra_m'], w['world'])
        for g, v in sorted(near_worst.items()):
            problems.append(f"{row['source']} {row['map']} {g} near {v:.3f} m, "
                            f"past its repaired {GATE_NEAR_LIMITS[(row['map'], g)]:.3f} m limit")
        for g, (v, at) in sorted(worst.items()):
            listed = known.get((row['source'], row['map'], g))
            if listed is None:
                problems.append(f"{row['source']} {row['map']} {g} {v:.3f} m at {[round(x, 3) for x in at]}")
            elif v > listed[0] + GATE_MAP_SLACK_M.get(row['map'], slack):
                problems.append(f"{row['source']} {row['map']} {g} {v:.3f} m at {[round(x, 3) for x in at]}, "
                                f"past its listed {listed[0]:.3f} m")
        # A placement or neighbour that could not be measured is not a pass: it would hide any gap it holds.
        side = objects.get('provider_unknown', {})
        for kind, rows in [('placement', objects.get('unknown', []))] + [(f'{k} drawing', side.get(k, []))
                                                                       for k in ('reference', 'source')]:
            for u in rows:
                name = u.get('name') or u.get('geometry')
                if kind == 'placement' and (row['source'], row['map'], name) in unmeasured:
                    continue
                problems.append(f"{row['source']} {row['map']} {kind} {name} {u.get('identity')} unmeasured: "
                                f"{u.get('reason')}")
        terrain = row.get('terrain', {})
        if terrain.get('error'):
            problems.append(f"{row['source']} {row['map']} terrain unmeasured: {terrain['error']}")
        elif (terrain.get('worst_excess_m') or 0) > ceiling:
            problems.append(f"{row['source']} {row['map']} terrain {terrain['worst_excess_m']:.3f} m at {terrain.get('worst_at')}")
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--case', action='append', required=True, metavar='NAME=ASSETS')
    parser.add_argument('--map', action='append')
    parser.add_argument('--samples', type=int, default=64)
    parser.add_argument('--no-vehicles', action='store_true')
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--gate', action='store_true',
                        help=f'measure only {", ".join(GATE_MAPS)} and fail on an unlisted in-boundary scene excess '
                             f'above {GATE_CEILING_M} m, a listed one grown past its measured worst by {GATE_SLACK_M} m, '
                             'or an unlisted unmeasured placement')
    args = parser.parse_args()
    if args.gate:
        if args.map:
            parser.error('--gate measures its own maps')
        args.map = list(GATE_MAPS)
    cases = [value.split('=', 1) for value in args.case]
    if any(len(c) != 2 or not all(c) for c in cases):
        parser.error('each case must be NAME=ASSETS')
    report = dict(schema=1, method=METHOD, margin_m=MARGIN_M, samples_per_surface=args.samples,
                  scene_witnesses_per_state=SCENE_WITNESSES, reference=str(args.reference.resolve()),
                  instrument_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  scope='diagnostic map selection' if args.map else 'all catalog maps',
                  coverage='sampled; scene composition at rest; no camera, walking-route or all-pose claim',
                  cases={}, maps=[], vehicles=[])
    for name, folder in cases:
        folder = Path(folder)
        report['cases'][name] = str(folder.resolve())
        for key in args.map or REQUIRED_MAPS:
            row = dict(source=name, map=key)
            try:
                reference, source = Scene(args.reference, key, reference_art=True), Scene(folder, key)
                row['objects'] = sweep_objects(reference, source, args.samples)
            except (ValueError, KeyError, TypeError, OSError, IndexError) as error:
                row['objects'] = dict(error=str(error))
            row['terrain'] = sweep_terrain(args.reference, folder, key)
            report['maps'].append(row)
            print(name, key, len(row['objects'].get('witnesses', [])), row['terrain']['worst_excess_m'], flush=True)
        if not args.no_vehicles:
            for vehicle in sweep_vehicles(args.reference, folder, args.samples):
                report['vehicles'].append(dict(vehicle, source=name))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    if args.gate:
        report['gate'] = dict(ceiling_m=GATE_CEILING_M, slack_m=GATE_SLACK_M,
                              near_limits=[list(k) + [v] for k, v in GATE_NEAR_LIMITS.items()],
                              map_slack_m=GATE_MAP_SLACK_M,
                              known=[list(k) + list(v) for k, v in GATE_KNOWN.items()],
                              unmeasured=[list(k) + [why] for k, why in GATE_UNMEASURED.items()],
                              problems=gate_problems(report, GATE_CEILING_M, GATE_KNOWN, GATE_UNMEASURED))
        for problem in report['gate']['problems']:
            print('offender:', problem)
    args.out.write_text(json.dumps(report, indent=1) + '\n')
    return int(bool(args.gate and report['gate']['problems']))


if __name__ == '__main__':
    sys.exit(main())
