#!/usr/bin/env python3
"""Gate placed render art against corrected collision, relative to the PS2 art.

  render_collision_check.py --reference ASSETS --case xbox=ASSETS --out report.json

Reads baked GLBs/manifests only. Every base/layer placement is reported, including
visual-only objects and objects outside playable paths. Surface checks use a
fixed, deterministic sample of collision vertices, triangle centres and primitive
surfaces; nearest distances are to actual render triangles (not their bounds).
The PS2 render/collision residual is the baseline: authored collision is simpler
than art, so zero residual is not a valid general acceptance rule. A source fails
when a collision sample inside playable bounds is farther from its render art
than the reference art by --tolerance metres, when a render sample moves farther
from collision than its nearest reference-art point by that tolerance, or when
playable render bounds move farther than that. Direct artwork-to-artwork distance
is reported separately; a different detail mesh does not alone fail the gate. This is a sampled regression gate, not a proof of continuous equivalence.
It never edits collision, render assets or the source trees. Reports contain
names, counts, distances and bounds, never mesh payloads. Normal player builds do
not require a reference disc; this is a developer gate over independent bakes.
"""
import argparse
from functools import lru_cache
import heapq
import hashlib
import json
import math
from pathlib import Path

from version_terrain_play import in_polygon
from world_fingerprint import glb_geometry, read_ron, safe_child

# Both base games' Instant Action catalog, using rebake.sh's two legacy aliases.
# Missing a catalog map must be unmeasured, not silently absent from a gate.
REQUIRED_MAPS = tuple(
    f'bf1_{name}' for name in 'bes1 bes2 end1 geo1 hot1 kam1 kas1 kas2 nab1 nab2 rhn1 rhn2 tat2 yav1 yav2'.split()
) + tuple(
    f'bf2_{name}' for name in 'cor1 dag1 dea1 end1 fel1 geo1 hot1 kam1 kas2 mus1 myg1 nab2 pol1 tan1 tat3 uta1 yav1 spa1 spa3 spa6 spa7 spa8 spa9'.split()
) + ('tat1', 'tat2')


def add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


def mul(a, s):
    return tuple(x * s for x in a)


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def placed(point, inst):
    # viewer::instance_matrix uses the three stored vectors as COLUMNS.
    r, t = inst['rotation'], inst['translation']
    return tuple(sum(r[c][i] * point[c] for c in range(3)) + t[i] for i in range(3))


def bounds(points):
    return [min(p[i] for p in points) for i in range(3)] + [max(p[i] for p in points) for i in range(3)]


def box_distance2(p, box):
    return sum(max(box[i] - p[i], 0, p[i] - box[i + 3]) ** 2 for i in range(3))


def segment_distance2(p, a, b):
    ab = sub(b, a)
    size = dot(ab, ab)
    t = max(0.0, min(1.0, dot(sub(p, a), ab) / size)) if size else 0
    d = sub(p, add(a, mul(ab, t)))
    return dot(d, d)


def triangle_closest(p, tri):
    """Closest point by triangle Voronoi region; degenerate faces use edges."""
    a, b, c = tri
    ab, ac, ap = sub(b, a), sub(c, a), sub(p, a)
    d1, d2 = dot(ab, ap), dot(ac, ap)
    if d1 <= 0 and d2 <= 0:
        return a
    bp = sub(p, b)
    d3, d4 = dot(ab, bp), dot(ac, bp)
    if d3 >= 0 and d4 <= d3:
        return b
    vc = d1 * d4 - d3 * d2
    if vc <= 0 and d1 >= 0 and d3 <= 0 and d1 != d3:
        return add(a, mul(ab, d1 / (d1 - d3)))
    cp = sub(p, c)
    d5, d6 = dot(ab, cp), dot(ac, cp)
    if d6 >= 0 and d5 <= d6:
        return c
    vb = d5 * d2 - d1 * d6
    if vb <= 0 and d2 >= 0 and d6 <= 0 and d2 != d6:
        return add(a, mul(ac, d2 / (d2 - d6)))
    va = d3 * d6 - d5 * d4
    edge = (d4 - d3) + (d5 - d6)
    if va <= 0 and d4 >= d3 and d5 >= d6 and edge:
        return add(b, mul(sub(c, b), (d4 - d3) / edge))
    denom = va + vb + vc
    if abs(denom) < 1e-24:
        candidates = []
        for x, y in ((a, b), (b, c), (c, a)):
            xy = sub(y, x)
            size = dot(xy, xy)
            t = max(0.0, min(1.0, dot(sub(p, x), xy) / size)) if size else 0
            candidates.append(add(x, mul(xy, t)))
        return min(candidates, key=lambda q: dot(sub(p, q), sub(p, q)))
    return add(a, add(mul(ab, vb / denom), mul(ac, vc / denom)))


def triangle_distance2(p, tri):
    d = sub(p, triangle_closest(p, tri))
    return dot(d, d)


class Surface:
    """Triangle BVH. Bounds prune queries; final distances always use faces."""
    def __init__(self, vertices, indices):
        if len(indices) % 3:
            raise ValueError('triangle index count is not divisible by three')
        self.vertices = [tuple(p) for p in vertices]
        if not self.vertices or any(len(p) != 3 or not all(math.isfinite(x) for x in p) for p in self.vertices):
            raise ValueError('invalid or nonfinite surface vertices')
        self.triangles = [tuple(self.vertices[int(i)] for i in indices[n:n + 3]) for n in range(0, len(indices), 3)]
        if not self.triangles:
            raise ValueError('empty triangle surface')
        self.bounds = bounds(self.vertices)
        self.nodes = []
        self._build(list(range(len(self.triangles))))

    def _build(self, faces):
        box = bounds([p for i in faces for p in self.triangles[i]])
        index = len(self.nodes)
        self.nodes.append(None)
        if len(faces) <= 12:
            self.nodes[index] = (box, faces, None)
        else:
            axis = max(range(3), key=lambda i: box[i + 3] - box[i])
            faces.sort(key=lambda i: sum(p[axis] for p in self.triangles[i]))
            middle = len(faces) // 2
            left, right = self._build(faces[:middle]), self._build(faces[middle:])
            self.nodes[index] = (box, left, right)
        return index

    def closest(self, point):
        best, nearest = math.inf, None
        queue = [(box_distance2(point, self.bounds), 0)]
        while queue:
            lower, index = heapq.heappop(queue)
            if lower > best:
                break
            _box, a, b = self.nodes[index]
            if b is None:
                for face in a:
                    q = triangle_closest(point, self.triangles[face])
                    d = dot(sub(point, q), sub(point, q))
                    if d < best:
                        best, nearest = d, q
            else:
                for child in (a, b):
                    d = box_distance2(point, self.nodes[child][0])
                    if d <= best:
                        heapq.heappush(queue, (d, child))
        return math.sqrt(max(0.0, best)), nearest

    def distance(self, point):
        return self.closest(point)[0]


def sample(points, count):
    # Evenly distributed deterministic indices, including both endpoints.
    if len(points) <= count:
        return points
    if count < 2:
        raise ValueError('sample count must be at least two')
    return [points[i * (len(points) - 1) // (count - 1)] for i in range(count)]


def surface_samples(surface, count):
    vertices = list(dict.fromkeys(surface.vertices))
    centres = [mul(add(add(*tri[:2]), tri[2]), 1 / 3) for tri in surface.triangles]
    # Both lists get a quota: duplicated vertex streams cannot crowd out faces.
    return sample(vertices, max(2, count // 2)) + sample(centres, max(2, count // 2))


@lru_cache(maxsize=256)
def surface(path):
    return Surface(*glb_geometry(Path(path)))


def require_rigid(transform):
    r, t = transform['rotation'], transform['translation']
    if len(r) != 3 or any(len(column) != 3 for column in r) or len(t) != 3:
        raise ValueError('invalid collision/placement transform')
    if not all(math.isfinite(x) for column in r for x in column) or not all(math.isfinite(x) for x in t):
        raise ValueError('nonfinite collision/placement transform')
    if any(abs(dot(r[i], r[j]) - (1 if i == j else 0)) > .001 for i in range(3) for j in range(3)):
        raise ValueError('nonrigid transform requires world-space surface measurement')


def primitive_samples(prim):
    require_rigid(prim)
    if len(prim['dims']) != 3 or any(not math.isfinite(x) or x < 0 for x in prim['dims']):
        raise ValueError('invalid collision primitive dimensions')
    x, y, z = prim['dims']
    kind = prim['kind']
    if kind == 'box':
        points = [(a * x, b * y, c * z) for a in (-1, 0, 1) for b in (-1, 0, 1) for c in (-1, 0, 1)
                  if abs(a) + abs(b) + abs(c) >= 1 and (abs(a) == 1 or abs(b) == 1 or abs(c) == 1)]
    elif kind == 'sphere':
        points = []
        for latitude in range(1, 8):
            angle = math.pi * latitude / 8
            for longitude in range(16):
                az = math.tau * longitude / 16
                points.append((x * math.sin(angle) * math.cos(az), x * math.cos(angle), x * math.sin(angle) * math.sin(az)))
        points += [(0, x, 0), (0, -x, 0)]
    elif kind == 'cylinder':
        points = [(x * math.cos(math.tau * i / 16), h, x * math.sin(math.tau * i / 16))
                  for h in (-y / 2, 0, y / 2) for i in range(16)] + [(0, -y / 2, 0), (0, y / 2, 0)]
    else:
        raise ValueError(f'unsupported collision primitive {kind}')
    return [placed(p, prim) for p in points]


def primitive_distance(point, prim):
    # The baked XFRM columns are the rotation of these rigid collision prims.
    delta = sub(point, prim['translation'])
    local = tuple(dot(column, delta) for column in prim['rotation'])
    dims, kind = prim['dims'], prim['kind']
    if kind == 'sphere':
        return abs(math.sqrt(dot(local, local)) - dims[0])
    if kind == 'box':
        q = [abs(local[i]) - dims[i] for i in range(3)]
    elif kind == 'cylinder':
        q = [math.hypot(local[0], local[2]) - dims[0], abs(local[1]) - dims[1] / 2]
    else:
        raise ValueError(f'unsupported collision primitive {kind}')
    return abs(math.sqrt(sum(max(x, 0) ** 2 for x in q)) + min(max(q), 0))


def merge_named(primary, secondary, key):
    result = {}
    for row in list(primary) + list(secondary):
        result.setdefault(str(row[key]).lower(), row)
    return result


class Scene:
    def __init__(self, assets, key):
        self.folder = assets / key
        self.doc = read_ron(self.folder / 'manifest.ron')
        game = self.doc['source_game']
        common_folder = assets / f'common_{game}'
        common = read_ron(common_folder / 'common.ron')
        self.classes = merge_named(self.doc.get('classes', []), common.get('classes', []), 'name')
        self.geometries = merge_named(self.doc.get('geometries', []), common.get('geometries', []), 'name')
        self.models = {p.stem.lower(): p for p in (common_folder / 'models').glob('*.glb')}
        self.models.update({p.stem.lower(): p for p in (self.folder / 'models').glob('*.glb')})
        self.collisions = {}
        for folder, document in ((self.folder, self.doc), (common_folder, common)):
            for row in document.get('collision', []):
                self.collisions.setdefault(str(row['geometry']).lower(), (folder, row))
        self.instances = {}
        self.layer_names = []
        # Names are not unique: Coruscant has three 'commondesign' wrappers.
        # A wrapper may also have a source-only sound world before the same
        # mode world; the runtime selects across ALL its world names. Preserve
        # each wrapper ordinal instead of dropping or mispairing those objects.
        worlds = [('base', self.doc)] + [(row['name'], row) for row in self.doc.get('layers', [])]
        for ordinal, (layer, world) in enumerate(worlds):
            self.layer_names.append(layer)
            for index, inst in enumerate(world.get('instances', [])):
                self.instances[(ordinal, index)] = inst
        self.boundary = [[(p[0], p[2]) for p, _r in path['nodes']] for path in self.doc.get('paths', [])
                         if path['name'].lower().startswith('boundary') and len(path['nodes']) >= 3]

    def resolve(self, inst):
        cls = self.classes.get(inst['class'].lower())
        if cls and cls.get('build_zone') and self.doc['source_game'] == 'bf2' and cls.get('base', '').lower() not in ('armedbuilding', 'armedbuildingdynamic'):
            cls = self.classes.get(cls['build_zone'].lower())
        if cls is None:
            raise ValueError(f"unresolved placed class {inst['class']}")
        geometry = cls.get('geometry')
        if not geometry:
            return None, None
        geometry = geometry.lower()
        model = self.geometries.get(geometry, {}).get('model', geometry).lower()
        return geometry, self.models.get(model)

    def in_play(self, point):
        return not self.boundary or any(in_polygon(poly, point[0], point[2]) for poly in self.boundary)

    @lru_cache(maxsize=512)
    def collision_identity(self, geometry):
        entry = self.collisions.get(geometry)
        if entry is None:
            return None
        folder, collision = entry
        # Geometry/node labels select a record but are not shape inputs once
        # resolved (game::assets::load_collision_geom). Primitive names remain:
        # class collision-layer rules resolve those names.
        shape = dict(prims=collision.get('prims', []), mesh=None)
        if collision.get('glb'):
            shape['mesh'] = glb_geometry(safe_child(folder, collision['glb']))
        return hashlib.sha256(json.dumps(shape, sort_keys=True, separators=(',', ':')).encode()).hexdigest()

    def collision_distance(self, geometry, point):
        folder, collision = self.collisions[geometry]
        distances = [primitive_distance(point, prim) for prim in collision.get('prims', [])]
        if collision.get('glb'):
            distances.append(surface(str(safe_child(folder, collision['glb']))).distance(point))
        if not distances:
            raise ValueError('empty collision surface')
        return min(distances)

    @lru_cache(maxsize=512)
    def collision_samples(self, geometry, count):
        entry = self.collisions.get(geometry)
        if entry is None:
            return [], 0
        folder, collision = entry
        points, triangles = [], 0
        if collision.get('glb'):
            s = surface(str(safe_child(folder, collision['glb'])))
            points.extend(surface_samples(s, count))
            triangles = len(s.triangles)
        for prim in collision.get('prims', []):
            points.extend(primitive_samples(prim))
        return points, triangles


def percentile(values, fraction):
    if not values:
        return None
    values = sorted(values)
    return values[min(len(values) - 1, int(fraction * (len(values) - 1)))]


def compare_map(reference, other, key, label, tolerance, samples):
    ref, src = Scene(reference, key), Scene(other, key)
    rows, cache = [], {}
    for identity, inst in ref.instances.items():
        ordinal, index = identity
        row = dict(layer=ref.layer_names[ordinal], layer_ordinal=ordinal, index=index,
                   source_layer=src.layer_names[ordinal] if ordinal < len(src.layer_names) else None,
                   name=inst.get('name', ''), class_name=inst['class'])
        rows.append(row)
        try:
            require_rigid(inst)
            geometry, ref_model = ref.resolve(inst)
            row['geometry'] = geometry
            src_inst = src.instances.get(identity)
            if src_inst is None:
                raise ValueError('placement missing from source')
            if any(src_inst.get(k) != inst.get(k) for k in ('class', 'rotation', 'translation')):
                raise ValueError('source placement differs from corrected reference')
            src_geom, src_model = src.resolve(src_inst)
            if geometry is None and src_geom is None:
                row['status'] = 'no_render_geometry'
                continue
            if geometry is None or src_geom is None:
                raise ValueError('source render geometry presence differs')
            row['collision_identical'] = ref.collision_identity(geometry) == src.collision_identity(src_geom)
            if not row['collision_identical']:
                raise ValueError('corrected source collision differs from PS2 reference')
            if ref_model is None or src_model is None:
                raise ValueError(f"missing render model: reference={ref_model is not None}, source={src_model is not None}")
            collision_points, triangles = ref.collision_samples(geometry, samples)
            row['collision_triangles'] = triangles
            if not collision_points:
                row['status'] = 'no_collision_surface'
                continue
            cache_key = (str(ref_model), str(src_model), geometry)
            if cache_key not in cache:
                ref_surface, src_surface = surface(str(ref_model)), surface(str(src_model))
                ref_d = [ref_surface.distance(p) for p in collision_points]
                src_d = ref_d if ref_model.read_bytes() == src_model.read_bytes() else [src_surface.distance(p) for p in collision_points]
                art_points = surface_samples(src_surface, samples)
                same_art = ref_model.read_bytes() == src_model.read_bytes()
                pairs = [(0.0, p) for p in art_points] if same_art else [ref_surface.closest(p) for p in art_points]
                art_d = [pair[0] for pair in pairs]
                reverse_delta = ([0.0] * len(art_points) if same_art else
                                 [ref.collision_distance(geometry, p) - ref.collision_distance(geometry, pair[1])
                                  for p, pair in zip(art_points, pairs)])
                cache[cache_key] = (ref_surface, src_surface, ref_d, src_d, art_points, art_d, reverse_delta)
            ref_surface, src_surface, ref_d, src_d, art_points, art_d, reverse_delta = cache[cache_key]
            chosen = [i for i, p in enumerate(collision_points) if ref.in_play(placed(p, inst))]
            row['surface_samples'] = len(collision_points)
            row['playable_samples'] = len(chosen)
            # Bounds are transformed from every render vertex, not from a local
            # AABB, so a rotated broad box cannot create an apparent mismatch.
            ref_world = [placed(p, inst) for p in ref_surface.vertices]
            src_world = [placed(p, src_inst) for p in src_surface.vertices]
            rb, sb = bounds(ref_world), bounds(src_world)
            row['reference_render_bounds'] = rb
            row['source_render_bounds'] = sb
            row['render_bounds_max_delta_m'] = max(abs(a - b) for a, b in zip(rb, sb))
            ref_play = [p for p in ref_world if ref.in_play(p)]
            src_play = [p for p in src_world if ref.in_play(p)]
            play_bounds = (max(abs(a - b) for a, b in zip(bounds(ref_play), bounds(src_play)))
                           if ref_play and src_play else None)
            row['playable_render_bounds_max_delta_m'] = play_bounds
            # Filtering vertices at a hard boundary is discontinuous: moving
            # one vertex by a fraction of a millimetre can remove a many-metre
            # extremum from that subset. Whole-envelope motion can confirm a
            # bounds regression; a stable envelope cannot rule out movement
            # of an inner piece, so ambiguous bounds remain unmeasured.
            bounds_regressed = (play_bounds is not None and play_bounds > tolerance
                                and row['render_bounds_max_delta_m'] > tolerance)
            row['render_bounds_regressed'] = bounds_regressed
            row['collision_to_reference_max_m'] = max((ref_d[i] for i in chosen), default=None)
            row['collision_to_source_max_m'] = max((src_d[i] for i in chosen), default=None)
            delta = [src_d[i] - ref_d[i] for i in chosen]
            row['surface_residual_increase_max_m'] = max(delta, default=None)
            row['surface_residual_increase_p95_m'] = percentile(delta, .95)
            row['surface_samples_regressed'] = sum(d > tolerance for d in delta)
            art_chosen = [i for i, p in enumerate(art_points) if ref.in_play(placed(p, inst))]
            art_delta = [art_d[i] for i in art_chosen]
            row['render_to_reference_samples'] = len(art_chosen)
            row['render_to_reference_max_m'] = max(art_delta, default=None)
            row['render_to_reference_p95_m'] = percentile(art_delta, .95)
            row['render_to_reference_samples_regressed'] = sum(d > tolerance for d in art_delta)
            reverse_play = [reverse_delta[i] for i in art_chosen]
            row['render_to_collision_residual_increase_max_m'] = max(reverse_play, default=None)
            row['render_to_collision_samples_regressed'] = sum(d > tolerance for d in reverse_play)
            reference_art_in_play = bool(ref_play) or any(ref.in_play(placed(p, inst)) for p in surface_samples(ref_surface, samples))
            source_art_in_play = bool(src_play) or bool(art_chosen)
            row['reference_art_in_play'] = reference_art_in_play
            row['source_art_in_play'] = source_art_in_play
            row['status'] = ('outside_playable_samples' if not chosen and not reference_art_in_play and not source_art_in_play else 'offender'
                             if reference_art_in_play != source_art_in_play or row['surface_samples_regressed'] or row['render_to_collision_samples_regressed']
                             or bounds_regressed else 'pass')
            if row['status'] == 'pass' and play_bounds is not None and play_bounds > tolerance:
                row.update(status='unmeasured', error='playable-vertex bounds are ambiguous; triangle clipping required')
        except (ValueError, OSError, KeyError, IndexError, TypeError) as exc:
            row.update(status='unmeasured', error=str(exc))
    extras = sorted(set(src.instances) - set(ref.instances))
    summary = {status: sum(r['status'] == status for r in rows) for status in sorted({r['status'] for r in rows})}
    return dict(source=label, map=key, placements=len(rows), extra_source_placements=len(extras), counts=summary, objects=rows)


def report_problems(report, reference, cases):
    """Validate a completed gate receipt independently of the helper exit code."""
    errors = []
    if not isinstance(report, dict) or report.get('schema') != 1 or report.get('complete') is not True:
        return ['missing complete schema-1 report']
    if not isinstance(report.get('reference'), str) or Path(report['reference']).resolve() != Path(reference).resolve():
        errors.append('reference assets do not match requested input')
    rows = report.get('maps')
    if not isinstance(rows, list) or not rows:
        return errors + ['report has no source/map rows']
    names = [case.split('=', 1)[0] for case in cases]
    if len(set(names)) != len(names) or any(not name for name in names):
        return errors + ['invalid or duplicate requested source names']
    keys = set(REQUIRED_MAPS) | {p.parent.name for p in Path(reference).glob('*/manifest.ron')}
    expected = {(name, key) for name in names for key in keys}
    actual = []
    statuses = {'pass', 'offender', 'unmeasured', 'no_collision_surface', 'no_render_geometry', 'outside_playable_samples'}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get('source'), str) or not isinstance(row.get('map'), str):
            errors.append('invalid source/map row')
            continue
        actual.append((row['source'], row['map']))
        counts = row.get('counts')
        if (not isinstance(counts, dict) or not counts or set(counts) - statuses
                or any(type(v) is not int or v < 0 for v in counts.values())):
            errors.append(f"invalid counts for {row['source']} {row['map']}")
            continue
        if type(row.get('extra_source_placements')) is not int or row['extra_source_placements'] < 0:
            errors.append('missing/invalid extra-placement count')
        if row.get('error'):
            # A failed map load is a complete attempted measurement, but can
            # never be clean. Such a row has no placed-object results.
            if not counts.get('unmeasured'):
                errors.append('map error was not counted as unmeasured')
            continue
        objects = row.get('objects')
        if (type(row.get('placements')) is not int or row['placements'] <= 0
                or not isinstance(objects, list) or len(objects) != row['placements']):
            errors.append('missing/empty/inconsistent placed-object results')
            continue
        measured = {}
        for obj in objects:
            status = obj.get('status') if isinstance(obj, dict) else None
            if status not in statuses:
                errors.append('unknown placed-object status')
                continue
            measured[status] = measured.get(status, 0) + 1
        if measured != {k: v for k, v in counts.items() if v}:
            errors.append('placed-object counts do not match results')
        if not (counts.get('pass') or counts.get('offender')):
            errors.append('map has no measured in-play surface results')
    if len(actual) != len(set(actual)):
        errors.append('duplicate source/map rows')
    if set(actual) != expected:
        errors.append(f'source/map coverage differs: {len(expected - set(actual))} missing, {len(set(actual) - expected)} extra')
    return errors


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--reference', type=Path, required=True)
    p.add_argument('--case', action='append', required=True, metavar='NAME=ASSETS')
    p.add_argument('--map', action='append', help='explicit scoped maps; default is the full 40-map catalog plus any extra reference maps')
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--table', type=Path)
    p.add_argument('--tolerance', type=float, default=.1, help='allowed regression relative to PS2 art, metres (default .1)')
    p.add_argument('--samples', type=int, default=256, help='max mesh samples per geometry; primitives sampled separately')
    args = p.parse_args()
    if not math.isfinite(args.tolerance) or args.tolerance < 0 or args.samples < 4:
        p.error('tolerance must be finite and nonnegative; samples must be >=4')
    try:
        cases = [(name, Path(folder)) for name, folder in (s.split('=', 1) for s in args.case)]
    except ValueError:
        p.error('each --case must be NAME=ASSETS')
    if any(not name for name, _folder in cases) or len({name for name, _folder in cases}) != len(cases):
        p.error('case names must be nonempty and unique')
    keys = args.map or sorted(set(REQUIRED_MAPS) | {path.parent.name for path in args.reference.glob('*/manifest.ron')})
    if not keys:
        p.error('reference has no maps')
    rows = []
    for name, folder in cases:
        for key in keys:
            try:
                row = compare_map(args.reference, folder, key, name, args.tolerance, args.samples)
            except (ValueError, OSError, KeyError, IndexError) as exc:
                row = dict(source=name, map=key, error=str(exc), counts={'unmeasured': 1}, placements=0, extra_source_placements=0)
            rows.append(row)
            print(name, key, row['counts'], flush=True)
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(json.dumps(dict(schema=1, complete=False, reference=str(args.reference),
                                    tolerance_m=args.tolerance, samples_per_mesh=args.samples,
                                    method='sampled_bidirectional_render_collision_differential', maps=rows), indent=2) + '\n')
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(dict(schema=1, complete=True, reference=str(args.reference), tolerance_m=args.tolerance,
                                      samples_per_mesh=args.samples, method='sampled_bidirectional_render_collision_differential', maps=rows), indent=2) + '\n')
    if args.table:
        lines = ['source,map,placements,pass,offender,unmeasured,visual_only,outside_playable_samples']
        for r in rows:
            c = r['counts']
            lines.append(','.join(str(v) for v in (r['source'], r['map'], r['placements'], c.get('pass', 0), c.get('offender', 0),
                         c.get('unmeasured', 0), c.get('no_collision_surface', 0) + c.get('no_render_geometry', 0), c.get('outside_playable_samples', 0))))
        args.table.parent.mkdir(parents=True, exist_ok=True)
        args.table.write_text('\n'.join(lines) + '\n')
    return int(any(r.get('error') or r['extra_source_placements'] or r['counts'].get('offender') or r['counts'].get('unmeasured') for r in rows))


if __name__ == '__main__':
    raise SystemExit(main())
