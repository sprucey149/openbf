#!/usr/bin/env python3
"""Fast art/contact measurements against PS2, in ordinary floating point.

Run --reference ASSETS --case xbox=ASSETS --out report.json. Optional --map and
--geometry narrow a diagnostic. No input is modified. The result is a sampled
measurement, never a complete-surface maximum or an all-pose acceptance claim.
The one-centimetre margin is added to PS2's own residual at each checked point.
It is small beside the 0.5 m soldier radius and 0.6 m step. Known candidates are
retained in alignment_known.py; listing one never suppresses a failure.
"""
import argparse
import hashlib
import json
from pathlib import Path

from render_collision_check import (Scene, REQUIRED_MAPS, placed, primitive_distance,
                                    primitive_samples, require_rigid, surface,
                                    surface_samples)
from render_contact_check import draw_surfaces, layer_bodies
from world_fingerprint import safe_child

def file_sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


MARGIN_M = .01
METHOD = 'ordinary_float_active_contact_samples'


class LayerContact:
    """Use each actual contact layer separately; never fill a hole with its union."""
    def __init__(self, scene, instance, layer):
        cls = scene.resolve_class(instance)
        geometry, _model = scene.resolve(instance)
        self.mesh, self.primitives = None, []
        if geometry not in scene.collisions or cls.get('base', '').lower() == 'trap':
            return
        folder, entry = scene.collisions[geometry]
        bodies = ([('collisionmesh', 0)] if entry.get('glb') else []) + [
            (str(p['name']).lower(), p.get('mask', 0)) for p in entry.get('prims', [])]
        if any(type(mask) not in (int, float) or not 0 <= mask <= 255 or int(mask) != mask for _name, mask in bodies):
            raise ValueError('invalid authored collision mask')
        # RON's numeric reader represents even integer-valued masks as floats.
        bodies = [(name, int(mask)) for name, mask in bodies]
        active = layer_bodies(bodies, cls.get('collision', []), layer)
        offset = int(bool(entry.get('glb')))
        if offset and active[0]:
            self.mesh = surface(str(safe_child(folder, entry['glb'])))
        self.primitives = [p for i, p in enumerate(entry.get('prims', []))
                           if active[offset + i] and p['kind'] in ('box', 'sphere', 'cylinder')]
        if any(active[offset + i] and p['kind'] not in ('box', 'sphere', 'cylinder')
               for i, p in enumerate(entry.get('prims', []))):
            raise ValueError('unsupported active collision primitive')

    def points(self, count):
        points = surface_samples(self.mesh, count) if self.mesh is not None else []
        for primitive in self.primitives:
            points.extend(primitive_samples(primitive))
        return points

    def distance(self, point):
        values = ([self.mesh.distance(point)] if self.mesh is not None else [])
        values.extend(primitive_distance(point, p) for p in self.primitives)
        if not values:
            raise ValueError('no active contact surface')
        return min(values)


def peak(values):
    return max(values, key=lambda row: row['extra_gap_m'], default=None)


def measure_layer(contact, reference_art, source_covered, source_potential, count):
    points = contact.points(count)
    if not points:
        return dict(status='no_active_contact', samples=0)
    if reference_art is None or source_covered is None or source_potential is None:
        return dict(status='unknown', samples=len(points), reason='no guaranteed drawn surface')
    forward = []
    for point in points:
        baseline, candidate = reference_art.distance(point), source_covered.distance(point)
        forward.append(dict(point=list(point), ps2_residual_m=baseline,
                            source_residual_m=candidate, extra_gap_m=candidate-baseline))
    reverse = []
    for point in surface_samples(source_potential, count):
        _distance, closest = reference_art.closest(point)
        baseline, candidate = contact.distance(closest), contact.distance(point)
        reverse.append(dict(point=list(point), ps2_point=list(closest), ps2_residual_m=baseline,
                            source_residual_m=candidate, extra_gap_m=candidate-baseline))
    worst = peak(forward + reverse)
    return dict(status='offender' if worst['extra_gap_m'] > MARGIN_M else 'sample_clear',
                samples=len(forward)+len(reverse), worst=worst,
                collision_to_art=peak(forward), art_to_collision=peak(reverse), _witnesses=forward+reverse)


def measure_map(reference_root, source_root, key, source_name, samples, wanted=()):
    reference = Scene(reference_root, key, reference_art=True)
    source = Scene(source_root, key)
    rows, cache = [], {}
    for identity, instance in reference.instances.items():
        geometry = None
        try:
            geometry, _model = reference.resolve(instance)
            if wanted and geometry not in wanted:
                continue
        except (ValueError, KeyError):
            if wanted:
                continue
        row = dict(layer=reference.layer_names[identity[0]], layer_ordinal=identity[0],
                   index=identity[1], name=instance.get('name'), geometry=geometry,
                   translation=instance['translation'])
        rows.append(row)
        try:
            other = source.instances[identity]
            require_rigid(instance)
            if instance != other:
                raise ValueError('source placement differs from PS2')
            source_geometry, _source_model = source.resolve(other)
            if (reference.collision_policy(instance) != source.collision_policy(other)
                    or reference.collision_identity(geometry) != source.collision_identity(source_geometry)):
                raise ValueError('source collision or layer policy differs from PS2')
            if geometry is None:
                row.update(status='no_render_geometry'); continue
            states = reference.render_states(geometry, instance)
            source_states = source.render_states(source_geometry, other)
            row['collision_sha256'] = reference.collision_identity(geometry)
            row['states'] = {}
            observations = []
            for state in sorted(states.keys() | source_states.keys()):
                a = states.get(state, states['model'])
                b = source_states.get(state, source_states['model'])
                if any(path is None for path in a+b):
                    raise ValueError('missing configured draw model')
                cache_key = (a, b, geometry, json.dumps(reference.collision_policy(instance), sort_keys=True))
                if cache_key not in cache:
                    _potential, baseline, reference_draws = draw_surfaces(reference, a)
                    potential, covered, draws = draw_surfaces(source, b)
                    layers = {layer: measure_layer(LayerContact(reference, instance, layer), baseline,
                                                  covered, potential, samples)
                              for layer in ('soldier', 'ordnance')}
                    partial = any('partial' in record['coverage'] for record in reference_draws+draws)
                    if partial:
                        for result in layers.values():
                            if result['status'] not in ('no_active_contact', 'unknown'):
                                result.update(status='unknown', reason='partial alpha coverage needs a visible-surface check; numeric candidates retained')
                    cache[cache_key] = dict(layers=layers, reference_draws=reference_draws, source_draws=draws,
                                           reference_model_sha256=[file_sha256(path) for path in a],
                                           source_model_sha256=[file_sha256(path) for path in b])
                result = cache[cache_key]
                observations.extend(w for layer in result['layers'].values() for w in layer.get('_witnesses', []))
                row['states'][state] = dict(result, layers={layer:{k:v for k,v in value.items() if not k.startswith('_')}
                                                          for layer,value in result['layers'].items()})
            layer_rows = [layer for state in row['states'].values() for layer in state['layers'].values()]
            row['status'] = ('unknown' if any(p['status']=='unknown' for p in layer_rows) else
                             'offender' if any(p['status']=='offender' for p in layer_rows) else
                             'sample_clear' if any(p['status']=='sample_clear' for p in layer_rows) else 'no_active_contact')
            reachable = [w for w in observations if reference.in_play(placed(w['point'], instance))]
            reachable_worst = peak(reachable)
            if reachable_worst:
                row['reachable_worst'] = dict(reachable_worst, world_point=list(placed(reachable_worst['point'], instance)))
                row['reachable_gap_max_m'] = max(w['source_residual_m'] for w in reachable)
            worst = peak(observations)
            if worst:
                row['worst'] = dict(worst, world_point=list(placed(worst['point'], instance)),
                                    in_playable_boundary=reference.in_play(placed(worst['point'], instance)))
        except (ValueError, KeyError, TypeError, OSError, IndexError) as error:
            row.update(status='unknown', reason=str(error))
    extra = sorted(set(source.instances)-set(reference.instances))
    counts = {status: sum(row['status']==status for row in rows) for status in sorted({row['status'] for row in rows})}
    reachable = peak([row['reachable_worst'] for row in rows if row.get('reachable_worst')])
    return dict(source=source_name, map=key, placements=len(rows), extra_source_placements=extra,
                counts=counts, objects=rows, reachable_worst=reachable,
                reachable_gap_max_m=max((row['reachable_gap_max_m'] for row in rows if row.get('reachable_gap_max_m') is not None),default=None))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--case', action='append', required=True, metavar='NAME=ASSETS')
    parser.add_argument('--map', action='append')
    parser.add_argument('--geometry', action='append', default=[])
    parser.add_argument('--samples', type=int, default=256)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--before', type=Path, help='earlier probe report for compact before/after rows')
    args = parser.parse_args()
    if args.samples < 4:
        parser.error('--samples must be at least four')
    try:
        cases = [value.split('=', 1) for value in args.case]
        if any(len(value)!=2 or not all(value) for value in cases) or len({value[0] for value in cases}) != len(cases):
            raise ValueError('each case must have a unique nonempty NAME=ASSETS')
        rows = []
        for name, folder in cases:
            for key in args.map or REQUIRED_MAPS:
                try:
                    row = measure_map(args.reference, Path(folder), key, name, args.samples, args.geometry)
                except (ValueError, KeyError, TypeError, OSError, IndexError) as error:
                    row = dict(source=name, map=key, placements=0, counts={'unknown':1}, objects=[], reason=str(error))
                rows.append(row)
                print(name, key, row['counts'], flush=True)
    except ValueError as error:
        parser.error(str(error))
    before = json.loads(args.before.read_text()) if args.before else {}
    if before and (before.get('method') != METHOD or before.get('margin_m') != MARGIN_M
                   or before.get('samples_per_surface') != args.samples
                   or before.get('geometry_selection', []) != args.geometry):
        parser.error('--before uses a different measurement method or margin')
    previous = {(row['source'],row['map']):row for row in before.get('maps',[])}
    compact = []
    for row in rows:
        old = previous.get((row['source'],row['map']), {})
        compact.append(dict(source=row['source'],map=row['map'],
                            before_gap_m=old.get('reachable_gap_max_m'), after_gap_m=row.get('reachable_gap_max_m'),
                            before_extra_m=(old.get('reachable_worst') or {}).get('extra_gap_m'),
                            after_extra_m=(row.get('reachable_worst') or {}).get('extra_gap_m'),
                            counts=row['counts'], worst_location=(row.get('reachable_worst') or {}).get('world_point')))
    from alignment_known import KNOWN
    report = dict(schema=1, method=METHOD, margin_m=MARGIN_M, samples_per_surface=args.samples,
                  geometry_selection=args.geometry,
                  scope='diagnostic selection' if args.map or args.geometry else 'all catalog maps',
                  coverage='deterministic per-model active-contact samples; no complete maximum, scene-occlusion, reachability path or all-pose claim',
                  reference=str(args.reference.resolve()), source_paths={name:str(Path(folder).resolve()) for name,folder in cases},
                  instrument_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  known_candidates=KNOWN, per_map=compact, maps=rows)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2)+'\n')
    return int(any(row.get('extra_source_placements') or not row['placements'] or
                   row['counts'].get('unknown') or row['counts'].get('offender') for row in rows))


if __name__ == '__main__':
    raise SystemExit(main())
