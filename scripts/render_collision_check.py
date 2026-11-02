#!/usr/bin/env python3
"""Gate placed render art against corrected collision, relative to the PS2 art.

  render_collision_check.py --reference ASSETS --case xbox=ASSETS --out report.json

Reads baked GLBs/manifests only. Every base/layer placement is reported, including
visual-only objects and objects outside playable paths. Acceptance requires a
continuous surface certificate, using triangle distance bounds and adaptive
subdivision. A work limit produces an unmeasured failure, never a sampled pass.
The PS2 render/collision residual is the baseline: authored collision is simpler
than art, so zero residual is not a valid general acceptance rule. A source fails
when a collision sample inside playable bounds is farther from its render art
than the reference art by --tolerance metres, when a render sample moves farther
from collision than its nearest reference-art point by that tolerance, or when
playable render bounds move farther than that. Direct artwork-to-artwork distance
is reported separately; different detail alone is not a physical offender.
Conservative sufficient bounds that cannot certify a detail mesh leave it
unmeasured. Whole objects are checked: a boundary does not establish invisibility.
It never edits collision, render assets or the source trees. Reports contain
names, counts, distances and bounds, never mesh payloads. Normal player builds do
not require a reference disc; this is a developer gate over independent bakes.
"""
import argparse
from functools import lru_cache
from fractions import Fraction
import heapq
import hashlib
import json
import math
from pathlib import Path
import time

SCHEMA = 2
METHOD = 'continuous_surface_certificates'
DEFAULT_MAX_CELLS = 100000
MAX_TOLERANCE_M = .1
DEFAULT_TOLERANCE_M = .01
MARGIN_REASON = ('0.01 m added contact-gap budget beyond the PS2 residual at the same collision point: '
                 'one centimetre is small relative to the current 0.5 m soldier radius and 0.6 m step, '
                 'and does not permit enlargement of gameplay collision. This is an alignment budget, not a float-error estimate. '
                 'Double-precision proof bounds separately include 1e-9 times local coordinate magnitude for roundoff.')

from version_terrain_play import in_polygon
from world_fingerprint import digest, glb_geometry, read_glb, read_ron, safe_child

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


def box_distance_lower(point, box):
    squared = sum(max(Fraction(box[i]) - Fraction(point[i]), 0,
                      Fraction(point[i]) - Fraction(box[i + 3])) ** 2 for i in range(3))
    value = math.sqrt(float(squared))
    return max(0., math.nextafter(math.nextafter(value, -math.inf), -math.inf))


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
            t = max(0, min(1, dot(sub(p, x), xy) / size)) if size else 0
            candidates.append(add(x, mul(xy, t)))
        return min(candidates, key=lambda q: dot(sub(p, q), sub(p, q)))
    return add(a, add(mul(ab, vb / denom), mul(ac, vc / denom)))


def triangle_distance2(p, tri):
    d = sub(p, triangle_closest(p, tri))
    return dot(d, d)


@lru_cache(maxsize=2048)
def rational_triangle(triangle):
    return tuple(tuple(Fraction(value) for value in point) for point in triangle)


def triangle_distance_upper(point, triangle):
    """Outward distance to one actual target face, valid if BVH chose badly.

    The exact rational closest point is on the selected closed triangle. Its
    distance is an upper bound on distance to the whole target surface. Two
    outward ULPs cover conversion of the exact squared norm and the square root.
    The general certificate additionally charges its coordinate roundoff guard.
    """
    exact_point = tuple(Fraction(value) for value in point)
    exact_triangle = rational_triangle(triangle)
    nearest = triangle_closest(exact_point, exact_triangle)
    delta = sub(exact_point, nearest)
    squared = dot(delta, delta)
    value = math.sqrt(float(squared))
    return math.nextafter(math.nextafter(value, math.inf), math.inf)


def _cross2(a, b, p):
    return (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])


def _area2(poly):
    return sum(a[0] * b[1] - a[1] * b[0] for a, b in zip(poly, poly[1:] + poly[:1]))


def _halfplane(poly, a, b, inside):
    if not poly:
        return []
    result = []
    previous = poly[-1]
    previous_d = _cross2(a, b, previous)
    for current in poly:
        current_d = _cross2(a, b, current)
        before = previous_d >= 0 if inside else previous_d <= 0
        after = current_d >= 0 if inside else current_d <= 0
        if before != after:
            t = previous_d / (previous_d - current_d)
            result.append(tuple(previous[i] + t * (current[i] - previous[i]) for i in range(2)))
        if after:
            result.append(current)
        previous, previous_d = current, current_d
    return result


def _subtract_triangle(poly, triangle):
    """Disjoint convex pieces of poly minus triangle, using exact rationals."""
    pieces = []
    remaining = poly
    if _area2(triangle) < 0:
        triangle = triangle[::-1]
    for a, b in zip(triangle, triangle[1:] + triangle[:1]):
        outside = _halfplane(remaining, a, b, False)
        if len(outside) >= 3 and _area2(outside) != 0:
            pieces.append(outside)
        remaining = _halfplane(remaining, a, b, True)
        if not remaining:
            break
    return pieces


class Surface:
    """Triangle BVH. Bounds prune queries; final distances always use faces."""
    def __init__(self, vertices, indices):
        if len(indices) % 3:
            raise ValueError('triangle index count is not divisible by three')
        self.vertices = [tuple(p) for p in vertices]
        if not self.vertices or any(len(p) != 3 or not all(math.isfinite(x) for x in p) for p in self.vertices):
            raise ValueError('invalid or nonfinite surface vertices')
        if any(type(i) is not int or i < 0 or i >= len(self.vertices) for i in indices):
            raise ValueError('invalid surface triangle index')
        self.triangles = [tuple(self.vertices[i] for i in indices[n:n + 3]) for n in range(0, len(indices), 3)]
        if not self.triangles:
            raise ValueError('empty triangle surface')
        # Accessors may retain unused vertices. Only indexed faces are drawn;
        # spare vertices must not expand bounds or masquerade as art samples.
        self.vertices = list(dict.fromkeys(p for triangle in self.triangles for p in triangle))
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

    def closest_face(self, point):
        best, nearest, selected = math.inf, None, None
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
                        best, nearest, selected = d, q, face
            else:
                for child in (a, b):
                    d = box_distance2(point, self.nodes[child][0])
                    if d <= best:
                        heapq.heappush(queue, (d, child))
        return math.sqrt(max(0.0, best)), nearest, selected

    def closest(self, point):
        distance, nearest, _face = self.closest_face(point)
        return distance, nearest

    def closest_triangle(self, point):
        distance, nearest, face = self.closest_face(point)
        return distance, nearest, self.triangles[face]

    def distance(self, point):
        return self.closest(point)[0]

    def nearby_faces(self, box):
        pending = [0]
        while pending:
            node_box, a, b = self.nodes[pending.pop()]
            if any(node_box[i] > box[i + 3] or node_box[i + 3] < box[i] for i in range(3)):
                continue
            if b is None:
                yield from a
            else:
                pending.extend((a, b))

    def triangle_union_bound(self, triangle, tolerance):
        """Prove full coverage by a union of target triangles near its plane.

        Drop the source plane's dominant axis and subtract every eligible
        projected target triangle. Coordinates and clipping predicates are exact
        rational representations of the decoded floats. There is no area
        threshold that can erase a small hole. At a covered projected point,
        the corresponding target point differs only along the dropped axis;
        its distance is bounded by the largest vertex-to-plane difference.
        """
        a, b, c = [tuple(Fraction(x) for x in point) for point in triangle]
        ab, ac = sub(b, a), sub(c, a)
        normal = (ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0])
        axis = max(range(3), key=lambda i: abs(normal[i]))
        if normal[axis] == 0 or tolerance < 0:
            return None
        keep = [i for i in range(3) if i != axis]
        projected = [tuple(p[i] for i in keep) for p in (a, b, c)]
        uncovered = [projected]
        box = bounds(triangle)
        box = [value + (-tolerance if i < 3 else tolerance) for i, value in enumerate(box)]
        threshold, upper, operations = Fraction(tolerance), Fraction(0), 0
        for count, face in enumerate(self.nearby_faces(box)):
            if count >= 2048:
                return None  # More work is unresolved, never approximate coverage.
            target = [tuple(Fraction(x) for x in point) for point in self.triangles[face]]
            distances = [abs(p[axis] - a[axis] + sum(normal[i] * (p[i] - a[i]) for i in keep) / normal[axis]) for p in target]
            distance = max(distances)
            if distance > threshold:
                continue
            clip = [tuple(p[i] for i in keep) for p in target]
            if _area2(clip) == 0:
                continue
            pieces = []
            for poly in uncovered:
                pieces.extend(_subtract_triangle(poly, clip))
                operations += 1
                if operations > 10000 or len(pieces) > 512:
                    return None
            uncovered = pieces
            upper = max(upper, distance)
            if not uncovered:
                return float(upper)
        return None


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


@lru_cache(maxsize=64)
def surface_union(paths):
    if len(paths) == 1:
        return surface(paths[0])
    vertices = [p for path in paths for triangle in surface(path).triangles for p in triangle]
    return Surface(vertices, list(range(len(vertices))))


@lru_cache(maxsize=2048)
def file_hash(path, modified, size):
    del modified, size
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def opacity_signature(path, scene):
    """Triangle geometry is insufficient when the shader can discard pixels.

    Equal alpha dependencies prove the same cutout in both builds. Different
    cutouts remain unmeasured until their actual visible surfaces are bounded.
    Hashing all RGBA bytes is deliberately conservative: RGB-only changes to a
    nonopaque material need an alpha-aware measurement rather than a false pass.
    """
    stat = path.stat()
    cache_key = (str(path), stat.st_mtime_ns, stat.st_size)
    if cache_key in scene.opacity_cache:
        return scene.opacity_cache[cache_key]
    doc, accessor = read_glb(path)
    selected = []
    for mesh in doc.get('meshes', []):
        for prim in mesh.get('primitives', []):
            mat = doc.get('materials', [])[prim['material']] if 'material' in prim else {}
            mode, extras = mat.get('alphaMode', 'OPAQUE'), mat.get('extras') or {}
            if mode not in ('OPAQUE', 'MASK', 'BLEND'):
                raise ValueError('unknown render alpha mode')
            if mode == 'OPAQUE' and not str(extras.get('light', '')).endswith('_add'):
                continue
            name = mat.get('name', '').lower()
            textures = []
            for suffix in (('.ktx2',) if scene.reference_art else ('.ktx2', '.tier2.ktx2')):
                candidate = next((folder / (name + suffix) for folder in scene.texture_dirs
                                  if (folder / (name + suffix)).is_file()), None)
                if candidate is not None:
                    stat = candidate.stat()
                    textures.append((suffix, file_hash(str(candidate), stat.st_mtime_ns, stat.st_size)))
            if not textures:
                raise ValueError(f'nonopaque material has no measured texture: {name}')
            attrs = prim.get('attributes', {})
            dependencies = {key: accessor(index) for key, index in attrs.items()
                            if key in ('POSITION', 'TEXCOORD_0', 'COLOR_0')}
            selected.append(dict(mode=mode, alpha_cutoff=mat.get('alphaCutoff'), light=extras.get('light'),
                                 uv_scroll=extras.get('uv_scroll'), textures=textures, attributes=dependencies,
                                 indices=accessor(prim['indices']) if 'indices' in prim else None))
    # Triangle-union distance cannot use a cutout face to fill a removed
    # opaque face. Until coverage resolves alpha texels, a model containing
    # nonopaque parts requires its complete indexed geometry to stay identical.
    result = digest(dict(nonopaque=selected, indexed_geometry=glb_geometry(path))) if selected else None
    scene.opacity_cache[cache_key] = result
    return result


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
    kind = prim['kind']
    used = {'box': (0, 1, 2), 'sphere': (0,), 'cylinder': (0, 1)}.get(kind)
    if used is None:
        raise ValueError(f'unsupported collision primitive {kind}')
    if len(prim['dims']) != 3 or any(not math.isfinite(prim['dims'][i]) or prim['dims'][i] < 0 for i in used):
        raise ValueError(f"invalid collision primitive dimensions: {kind} {prim.get('name', '')!r} {prim['dims']}")
    x, y, z = prim['dims']
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


def triangle_patch(triangle, identity):
    centre = mul(add(add(triangle[0], triangle[1]), triangle[2]), 1 / 3)
    radius = max(math.dist(centre, p) for p in triangle)
    return ('triangle', triangle, centre, radius, identity)


def split_patch(patch):
    kind, domain, _centre, _radius, identity = patch
    if kind == 'triangle':
        a, b = max(((0, 1), (1, 2), (2, 0)), key=lambda pair: math.dist(domain[pair[0]], domain[pair[1]]))
        c = 3 - a - b
        middle = mul(add(domain[a], domain[b]), .5)
        return [triangle_patch((domain[a], middle, domain[c]), identity),
                triangle_patch((middle, domain[b], domain[c]), identity)]
    point, radius_at, limits = domain
    u0, u1, v0, v1 = limits
    # Alternating parameter dimensions guarantees shrinking domains, including
    # polar patches whose angular dimensions have different physical scales.
    if identity[-1] % 2:
        middle = (v0 + v1) / 2
        halves = [(u0, u1, v0, middle), (u0, u1, middle, v1)]
    else:
        middle = (u0 + u1) / 2
        halves = [(u0, middle, v0, v1), (middle, u1, v0, v1)]
    return [parameter_patch(point, radius_at, half, identity[:-1] + (identity[-1] + 1,)) for half in halves]


def parameter_patch(point, radius_at, limits, identity):
    u0, u1, v0, v1 = limits
    centre = point((u0 + u1) / 2, (v0 + v1) / 2)
    return ('parameter', (point, radius_at, limits), centre, radius_at(limits), identity)


def primitive_patches(prim, index):
    """Cover the analytic primitive, rather than an inscribed polygon mesh.

    The radii bound every point of a parameter patch by arc length. They include
    cylinder caps and sphere poles. Rigid transforms preserve these metre bounds.
    """
    primitive_samples(prim)  # Validate dimensions, kind and the rigid frame.
    x, y, z = prim['dims']
    if prim['kind'] == 'box':
        for axis in range(3):
            other = [i for i in range(3) if i != axis]
            for sign in (-1, 1):
                corners = []
                for a, b in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
                    p = [0., 0., 0.]
                    p[axis] = sign * prim['dims'][axis]
                    p[other[0]], p[other[1]] = a * prim['dims'][other[0]], b * prim['dims'][other[1]]
                    corners.append(placed(p, prim))
                for tri in ((0, 1, 2), (0, 2, 3)):
                    yield triangle_patch(tuple(corners[i] for i in tri), ('primitive', index, axis, sign))
    elif prim['kind'] == 'sphere':
        def point(theta, phi):
            return placed((x * math.sin(theta) * math.cos(phi), x * math.cos(theta), x * math.sin(theta) * math.sin(phi)), prim)

        def radius(limits):
            u0, u1, v0, v1 = limits
            return x * ((u1 - u0) + (v1 - v0)) / 2
        for i in range(8):
            for j in range(16):
                yield parameter_patch(point, radius, (math.pi * i / 8, math.pi * (i + 1) / 8,
                                                     math.tau * j / 16, math.tau * (j + 1) / 16), ('sphere', index, 0))
    else:
        def side(angle, height):
            return placed((x * math.cos(angle), height, x * math.sin(angle)), prim)

        def side_radius(limits):
            u0, u1, v0, v1 = limits
            return x * (u1 - u0) / 2 + (v1 - v0) / 2
        for j in range(16):
            angles = (math.tau * j / 16, math.tau * (j + 1) / 16)
            yield parameter_patch(side, side_radius, (*angles, -y / 2, y / 2), ('cylinder_side', index, 0))
            for height in (-y / 2, y / 2):
                def cap(angle, radius, height=height):
                    return placed((radius * math.cos(angle), height, radius * math.sin(angle)), prim)

                def cap_radius(limits):
                    u0, u1, v0, v1 = limits
                    return v1 * (u1 - u0) / 2 + (v1 - v0) / 2
                yield parameter_patch(cap, cap_radius, (*angles, 0, x), ('cylinder_cap', index, 0))


def certify_surface(patches, evaluate, lipschitz, tolerance, max_cells, convex_target=None, witness_evaluate=None):
    """Fail-closed continuous bound, not a fixed-rate sampling assertion.

    evaluate returns (a sufficient upper-bound function, actual residual,
    witness details). Distance to any closed surface is 1-Lipschitz; a
    difference of distances is 2-Lipschitz. Every patch is either bounded
    completely, witnessed to fail, or explicitly unresolved. A finite work
    budget cannot turn unresolved patches into passes.
    """
    cells = queries = certified = 0
    observed, witnessed, upper_max = -math.inf, None, -math.inf
    values = {}
    originals = list(patches)
    # A difficult curved prefix must not consume the complete proof budget
    # before a later original face's obvious gap is even observed. This pass
    # can witness failure only: it never supplies an acceptance certificate.
    prepass_started = time.perf_counter()
    for original_index, patch in enumerate(originals):
        kind, domain, centre, _radius, identity = patch
        if kind == 'triangle':
            points = [centre, *domain]
        else:
            point_at, _radius_at, limits = domain
            u0, u1, v0, v1 = limits
            points = [centre, *(point_at(u, v) for u in (u0, u1) for v in (v0, v1))]
        for point in points:
            if witness_evaluate is not None:
                actual, detail = witness_evaluate(point)
                queries += 1
            else:
                if point not in values:
                    values[point] = evaluate(point)
                    queries += 1
                _bound, actual, detail = values[point]
            if not math.isfinite(actual):
                raise ValueError('nonfinite continuous surface witness')
            if actual > observed:
                observed = actual
                witnessed = dict(point=list(point), domain=list(identity), **detail)
            epsilon = 1e-9 * max(1., *(abs(value) for value in point))
            if actual > tolerance + epsilon:
                return dict(status='offender', cells=0, queries=queries, certified_patches=0,
                            maximum_observed_m=observed, witness=witnessed, complete=False,
                            prepass_patches=original_index + 1, prepass_seconds=time.perf_counter() - prepass_started)
        if len(values) > 32768:
            values.clear()
    prepass_seconds = time.perf_counter() - prepass_started
    for original in originals:
        pending = [original]
        while pending:
            if cells >= max_cells:
                return dict(status='unmeasured', reason='continuous certificate work limit', cells=cells,
                            queries=queries, certified_patches=certified, maximum_observed_m=observed if witnessed else None,
                            witness=witnessed, complete=False, prepass_patches=len(originals), prepass_seconds=prepass_seconds)
            patch = pending.pop()
            kind, domain, centre, radius, identity = patch
            cells += 1
            points = [centre] + (list(domain) if kind == 'triangle' else [])
            results = []
            for point in points:
                if point not in values:
                    values[point] = evaluate(point)
                    queries += 1
                bound, actual, detail = values[point]
                if not math.isfinite(bound) or not math.isfinite(actual):
                    raise ValueError('nonfinite continuous surface residual')
                if actual > observed:
                    observed = actual
                    witnessed = dict(point=list(point), domain=list(identity), **detail)
                results.append((point, bound, actual))
            epsilon = 1e-9 * max(1., *(abs(x) for p in points for x in p))
            if observed > tolerance + epsilon:
                return dict(status='offender', cells=cells, queries=queries, certified_patches=certified,
                            maximum_observed_m=observed, witness=witnessed, complete=False,
                            prepass_patches=len(originals), prepass_seconds=prepass_seconds)
            upper = results[0][1] + lipschitz * radius
            if kind == 'triangle':
                upper = min(upper, *(value + lipschitz * max(math.dist(p, q) for q in domain)
                                     for p, value, _actual in results[1:]))
                if convex_target is not None:
                    # Distance to one convex triangle is convex. A target face
                    # bounding all three vertices bounds the complete triangle.
                    _distance, _point, target_triangle = convex_target.closest_triangle(centre)
                    upper = min(upper, max(triangle_distance_upper(p, target_triangle) for p in domain))
                    if upper + epsilon > tolerance:
                        union_upper = convex_target.triangle_union_bound(domain, tolerance - epsilon)
                        if union_upper is not None:
                            upper = min(upper, union_upper)
            if upper + epsilon <= tolerance:
                certified += 1
                upper_max = max(upper_max, upper + epsilon)
            elif results[0][1] > tolerance + epsilon and results[0][2] <= tolerance:
                # The sufficient bound itself is too strict at this point.
                # More subdivisions cannot certify this domain using it.
                return dict(status='unmeasured', reason='conservative reverse bound cannot certify donor detail', cells=cells,
                            queries=queries, certified_patches=certified, maximum_observed_m=observed,
                            witness=witnessed, complete=False, prepass_patches=len(originals), prepass_seconds=prepass_seconds)
            elif radius <= epsilon:
                return dict(status='unmeasured', reason='residual is on the tolerance/roundoff boundary', cells=cells,
                            queries=queries, certified_patches=certified, maximum_observed_m=observed,
                            witness=witnessed, complete=False, prepass_patches=len(originals), prepass_seconds=prepass_seconds)
            else:
                pending.extend(split_patch(patch))
            if len(values) > 32768:
                values.clear()
    return dict(status='pass', cells=cells, queries=queries, certified_patches=certified,
                maximum_observed_m=observed if witnessed else None, certified_upper_m=upper_max if witnessed else None,
                witness=witnessed, complete=True, prepass_patches=len(originals), prepass_seconds=prepass_seconds)


def art_certificate(domain, target, tolerance, max_cells):
    def evaluate(p):
        distance, _nearest, triangle = target.closest_triangle(p)
        return triangle_distance_upper(p, triangle), distance, dict(art_distance_m=distance)
    def witness(p):
        distance = target.distance(p)
        return distance, dict(art_distance_m=distance)
    certificate = certify_surface((triangle_patch(tri, ('art_triangle', i)) for i, tri in enumerate(domain.triangles)),
                                  evaluate, 1., tolerance, max_cells, convex_target=target, witness_evaluate=witness)
    certificate['method'] = 'continuous_directed_hausdorff'
    return certificate


def continuous_alignment(ref_scene, geometry, reference, source, tolerance, max_cells, collision_work=True):
    """Sufficient art bounds first; exact collision residual where necessary."""
    if reference.triangles == source.triangles:
        exact = dict(status='pass', method='identical_indexed_surface', complete=True, maximum_observed_m=0., certified_upper_m=0., cells=0, queries=0)
        return dict(complete=True, status='pass', forward=exact, reverse=exact, art_forward=exact, art_reverse=exact)
    forward_art = art_certificate(reference, source, tolerance, max_cells)
    if forward_art['status'] == 'pass':
        forward = dict(forward_art, method='reference_to_source_hausdorff_sufficient_bound')
    elif collision_work:
        def forward_value(p):
            ref_d, src_d = reference.distance(p), source.distance(p)
            delta = src_d - ref_d
            _distance, _nearest, triangle = source.closest_triangle(p)
            upper = triangle_distance_upper(p, triangle) - box_distance_lower(p, reference.bounds)
            return upper, delta, dict(reference_residual_m=ref_d, source_residual_m=src_d)
        def forward_witness(p):
            a, b = reference.distance(p), source.distance(p)
            return b - a, dict(reference_residual_m=a, source_residual_m=b)
        # d_source - d_reference <= d_source, so the convex target-face
        # upper bound is also sufficient for this differential residual.
        forward = certify_surface(ref_scene.collision_patches(geometry), forward_value, 2., tolerance, max_cells,
                                  convex_target=source, witness_evaluate=forward_witness)
        forward['method'] = 'pointwise_collision_residual_difference'
    else:
        forward = dict(status='unmeasured', complete=False, reason='local differential not maximized; scene-surface proof required')
    reverse_art = art_certificate(source, reference, tolerance, max_cells)
    def reverse_value(p):
        art_d, nearest = reference.closest(p)
        source_d = ref_scene.collision_distance(geometry, p)
        reference_d = ref_scene.collision_distance(geometry, nearest)
        _distance, _nearest, triangle = reference.closest_triangle(p)
        upper = triangle_distance_upper(p, triangle)
        return upper, source_d - reference_d, dict(reference_residual_m=reference_d, source_residual_m=source_d,
                                                                reference_point=list(nearest), art_distance_m=art_d)
    if reverse_art['status'] == 'pass':
        reverse = dict(reverse_art, method='source_to_reference_hausdorff_sufficient_bound')
    elif collision_work:
        reverse = certify_surface((triangle_patch(tri, ('art_triangle', i)) for i, tri in enumerate(source.triangles)),
                                  reverse_value, 1., tolerance, max_cells, convex_target=reference)
        reverse['method'] = 'reference_art_distance_sufficient_bound'
    else:
        reverse = dict(status='unmeasured', complete=False, reason='local differential not maximized; scene-surface proof required')
    statuses = [forward['status'], reverse['status']]
    status = 'offender' if 'offender' in statuses else 'unmeasured' if 'unmeasured' in statuses else 'pass'
    return dict(complete=status == 'pass', status=status, forward=forward, reverse=reverse,
                art_forward=forward_art, art_reverse=reverse_art)


def merge_named(primary, secondary, key):
    result = {}
    for row in list(primary) + list(secondary):
        result.setdefault(str(row[key]).lower(), row)
    return result


class Scene:
    def __init__(self, assets, key, reference_art=False):
        self.assets = assets
        self.reference_art = reference_art
        self.folder = assets / key
        self.doc = read_ron(self.folder / 'manifest.ron')
        game = self.doc['source_game']
        common_folder = assets / f'common_{game}'
        common = read_ron(common_folder / 'common.ron')
        self.texture_dirs = (self.folder / 'textures', common_folder / 'textures')
        self.opacity_cache = {}
        self.raw_class_inputs = None
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

    def resolve_class(self, inst):
        cls = self.classes.get(inst['class'].lower())
        if cls and cls.get('build_zone') and self.doc['source_game'] == 'bf2' and cls.get('base', '').lower() not in ('armedbuilding', 'armedbuildingdynamic'):
            cls = self.classes.get(cls['build_zone'].lower())
        if cls is None:
            raise ValueError(f"unresolved placed class {inst['class']}")
        return cls

    def collision_policy(self, inst):
        cls = self.resolve_class(inst)
        base = str(cls.get('base', '')).lower()
        door = cls.get('door')
        if door is not None:
            # assets::collision_parts consumes the trigger positions/radii,
            # frame count and primitive tracks. Door parts and sounds affect
            # drawing/audio; the attachment companion checks those bindings.
            door = dict(frames=door.get('frames'),
                        triggers=[{key: value.get(key) for key in ('position', 'radius')}
                                  for value in door.get('triggers', [])],
                        prims=[dict(name=str(value.get('name', '')).lower(), motion=value.get('motion'))
                               for value in door.get('prims', [])])
        rig = cls.get('turret_rig')
        # assets::armed_node/vehicle_pads::posed_parts use the named node
        # hierarchy and bind matrices. The parts array binds render submeshes.
        nodes = ([dict(name=str(node.get('name', '')).lower(), parent=node.get('parent'), bind=node.get('bind'))
                  for node in rig.get('nodes', [])] if rig is not None and base == 'armedbuilding' else None)
        return dict(base=base, collision=[(str(layer).lower(), str(body).lower()) for layer, body in cls.get('collision', [])],
                    door=door, armed_nodes=nodes)

    def resolve(self, inst):
        cls = self.resolve_class(inst)
        geometry = cls.get('geometry')
        if not geometry:
            return None, None
        geometry = geometry.lower()
        model = self.geometries.get(geometry, {}).get('model', geometry).lower()
        return geometry, self.models.get(model)

    def class_inputs(self):
        if self.raw_class_inputs is None:
            from render_class_inputs import ClassInputs
            self.raw_class_inputs = ClassInputs(self.assets, self.folder, self.doc['source_game'], reader=read_ron)
        return self.raw_class_inputs

    def borrow_lifetime(self, inst):
        try:
            cls = self.resolve_class(inst)
            if cls.get('door') or cls.get('turret_rig'):
                return dict(eligible=False, reason='placed model has a moving door or rig')
            return self.class_inputs().static_lifetime([inst['class'], cls['name']])
        except (ValueError, OSError, KeyError, TypeError) as exc:
            return dict(eligible=False, reason=str(exc))

    def in_play(self, point):
        return not self.boundary or any(in_polygon(poly, point[0], point[2]) for poly in self.boundary)

    def render_states(self, geometry):
        g = self.geometries.get(geometry, {})
        model = str(g.get('model', geometry)).lower()
        lowd = next((str(name).lower() for tier, name in g.get('lods', []) if tier.lower() == 'lowd'), model)
        states = {'model': (self.models.get(model),), 'lowd': (self.models.get(lowd),)}
        if g.get('far'):
            states['model_with_far'] = (self.models.get(model), self.models.get(str(g['far']).lower()))
        return states

    def lod_switch(self, inst, geometry):
        states = self.render_states(geometry)
        if all(paths == states['model'] for paths in states.values()):
            return None
        path = states['model'][0]
        if path is None:
            raise ValueError('LOD switch model missing')
        doc, _accessor = read_glb(path)
        spheres = [mesh.get('extras', {}).get('sphere') for mesh in doc.get('meshes', [])]
        sphere = next((value for value in spheres if value is not None), None)
        if sphere is None:
            vertices, _indices = glb_geometry(path)
            box = bounds(vertices)
            sphere = [box, 'runtime bounds fallback']
        cls = self.classes.get(inst['class'].lower(), {})
        lighting = self.doc.get('lighting') or {}
        return digest(dict(sphere=sphere, near=lighting.get('near_scene_range'), far=lighting.get('far_scene_range'),
                           attach=cls.get('attach'), hardpoints=cls.get('hardpoints')))

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

    def collision_patches(self, geometry):
        folder, collision = self.collisions[geometry]
        if collision.get('glb'):
            for i, triangle in enumerate(surface(str(safe_child(folder, collision['glb']))).triangles):
                yield triangle_patch(triangle, ('collision_triangle', i))
        for i, prim in enumerate(collision.get('prims', [])):
            yield from primitive_patches(prim, i)


def percentile(values, fraction):
    if not values:
        return None
    values = sorted(values)
    return values[min(len(values) - 1, int(fraction * (len(values) - 1)))]


def unresolved_class_baseline(reference, source, identity):
    """Prove a shared missing direct class without dropping its placement.

    assets::resolve_class makes the ordinary renderer and CollisionWorld skip
    this row. Raw ClassDb consumers (pads/stations) and MapLayout's independent
    command-post parser are covered by identical raw classes, instance props,
    regions and paths. Procedural/attached art still requires its companion.
    """
    inst, other = reference.instances[identity], source.instances.get(identity)
    if other != inst or inst['class'].lower() in reference.classes or inst['class'].lower() in source.classes:
        return None
    if reference.doc.get('source_game') != source.doc.get('source_game'):
        return None
    from render_class_inputs import fnv
    def prop(name):
        key = fnv(name)
        return next((value for code, value in inst.get('props', []) if int(code) == key), None)
    capture, control, spawn = prop('CaptureRegion'), prop('ControlRegion'), prop('SpawnPath')
    space_post = inst['class'].lower() == 'com_bldg_controlzone_ctf' and spawn is not None
    # MapLayout only consults regions/paths for this instance if it is a post.
    # Unrelated source-only audio/sky regions cannot turn an absent flag mesh
    # into a renderer or collision consumer.
    is_post = capture is not None or space_post
    dependency_names = [value for value in (capture, control) if value is not None] if is_post else []
    def layout_inputs(scene):
        regions = [next((row for row in scene.doc.get('regions', []) if row['name'].lower() == str(name).lower()), None)
                   for name in dependency_names]
        path = next((row for row in scene.doc.get('paths', []) if row['name'].lower() == str(spawn).lower()), None) if is_post and spawn is not None else None
        return dict(regions=regions, spawn_path=path)
    layout_a, layout_b = layout_inputs(reference), layout_inputs(source)
    if layout_a != layout_b:
        return None
    name = inst['class'].lower()
    for scene in (reference, source):
        # Leaf/grass draws have their own class lookups and can render even
        # when ordinary Scene::resolve_class returns None.
        leaf = scene.doc.get('leaf') or []
        grass = (scene.doc.get('foliage') or {}).get('grass', [])
        if any(str(row.get('name', '')).lower() == name for row in list(leaf) + list(grass)):
            return None
    a, b = reference.class_inputs(), source.class_inputs()
    consumer_a, consumer_b = a.missing_direct_class_consumers(inst['class']), b.missing_direct_class_consumers(inst['class'])
    if not consumer_a['skip'] or not consumer_b['skip']:
        raise ValueError(f"unresolved class has a raw dynamic consumer: reference={consumer_a['engine_base']}, source={consumer_b['engine_base']}")
    return dict(status='baseline_runtime_skip', geometry=None, class_collision_identical=True,
                resolved_class_absent_both=True, raw_class_inputs_identical=a.sha256 == b.sha256, raw_class_inputs_sha256=a.sha256,
                raw_class_files=a.files, runtime_consumer_skip_proven=True,
                layout_inputs_identical=True, layout_inputs_sha256=digest(layout_a),
                raw_consumer_proof={'reference': consumer_a, 'source': consumer_b},
                baseline='PS2 and source both skip ordinary world art/collision; raw station/turret consumers also skip; '
                'referenced layout inputs are identical; procedural/attached art requires the companion receipt')


def absent_render_baseline(reference, source, identity, geometry, source_geometry):
    inst = reference.instances[identity]
    if source.instances.get(identity) != inst:
        return None
    a, b = reference.class_inputs(), source.class_inputs()
    if a.sha256 != b.sha256:
        return None
    collision = reference.collision_identity(geometry)
    if collision != source.collision_identity(source_geometry):
        return None
    return dict(status='baseline_collision_only' if collision is not None else 'baseline_no_render_geometry',
                render_absent_both=True, collision_identical=True, class_collision_identical=True,
                raw_class_inputs_identical=True, raw_class_inputs_sha256=a.sha256, raw_class_files=a.files,
                baseline='PS2 and source both resolve no direct drawn model; identical collision and raw class inputs '
                'retain this invisible-body/skipped-model baseline; indirect art requires the companion receipt')


def compare_map(reference, other, key, label, tolerance, samples, max_cells=DEFAULT_MAX_CELLS, progress=None):
    ref, src = Scene(reference, key, reference_art=True), Scene(other, key)
    rows, cache = [], {}
    for identity, inst in ref.instances.items():
        started = time.perf_counter()
        ordinal, index = identity
        row = dict(layer=ref.layer_names[ordinal], layer_ordinal=ordinal, index=index,
                   source_layer=src.layer_names[ordinal] if ordinal < len(src.layer_names) else None,
                   name=inst.get('name', ''), class_name=inst['class'], translation=inst['translation'])
        rows.append(row)
        try:
            require_rigid(inst)
            geometry, ref_model = ref.resolve(inst)
            row['geometry'] = geometry
            row['translation'] = inst['translation']
            src_inst = src.instances.get(identity)
            if src_inst is None:
                raise ValueError('placement missing from source')
            if any(src_inst.get(k) != inst.get(k) for k in ('name', 'class', 'rotation', 'translation', 'props')):
                raise ValueError('source placement differs from corrected reference')
            src_geom, src_model = src.resolve(src_inst)
            row['class_collision_identical'] = ref.collision_policy(inst) == src.collision_policy(src_inst)
            if not row['class_collision_identical']:
                raise ValueError('resolved source class collision-layer or motion policy differs from PS2')
            if geometry is None and src_geom is None:
                row.update(status='baseline_no_render_geometry', baseline='PS2 and source both resolve no render or geometry collision')
                continue
            if geometry is None or src_geom is None:
                raise ValueError('source render geometry presence differs')
            row['collision_identical'] = ref.collision_identity(geometry) == src.collision_identity(src_geom)
            if not row['collision_identical']:
                raise ValueError('corrected source collision differs from PS2 reference')
            if ref_model is None or src_model is None:
                if ref_model is None and src_model is None:
                    baseline = absent_render_baseline(ref, src, identity, geometry, src_geom)
                    if baseline is not None:
                        row.update(baseline)
                        continue
                raise ValueError(f"missing render model: reference={ref_model is not None}, source={src_model is not None}")
            cache_key = (str(ref_model), str(src_model), geometry)
            if cache_key not in cache:
                model_started = time.perf_counter()
                ref_surface, src_surface = surface(str(ref_model)), surface(str(src_model))
                collision_points, triangles = ref.collision_samples(geometry, samples)
                if not collision_points:
                    if ref_surface.triangles == src_surface.triangles:
                        visual = dict(status='pass', complete=True, method='identical_indexed_surface', maximum_observed_m=0.)
                    else:
                        a = art_certificate(ref_surface, src_surface, tolerance, max_cells)
                        b = art_certificate(src_surface, ref_surface, tolerance, max_cells)
                        visual = dict(complete=a['status'] == b['status'] == 'pass',
                                      status='pass' if a['status'] == b['status'] == 'pass' else 'unmeasured',
                                      method='visual_only_bidirectional_hausdorff', reference_to_source=a, source_to_reference=b)
                    cache[cache_key] = (ref_surface, src_surface, [], [], [], [], [], [], triangles, visual)
                else:
                    ref_d = [ref_surface.distance(p) for p in collision_points]
                    src_d = ref_d if ref_model.read_bytes() == src_model.read_bytes() else [src_surface.distance(p) for p in collision_points]
                    art_points = surface_samples(src_surface, samples)
                    same_art = ref_surface.triangles == src_surface.triangles
                    pairs = [(0.0, p) for p in art_points] if same_art else [ref_surface.closest(p) for p in art_points]
                    art_d = [pair[0] for pair in pairs]
                    reverse_delta = ([0.0] * len(art_points) if same_art else
                                     [ref.collision_distance(geometry, p) - ref.collision_distance(geometry, pair[1])
                                      for p, pair in zip(art_points, pairs)])
                    certificate = continuous_alignment(ref, geometry, ref_surface, src_surface, tolerance, max_cells, collision_work=False)
                    cache[cache_key] = (ref_surface, src_surface, collision_points, ref_d, src_d, art_points, art_d, reverse_delta, triangles, certificate)
                if progress:
                    progress(f'{label} {key} model {geometry} {time.perf_counter() - model_started:.3f}s')
            ref_surface, src_surface, collision_points, ref_d, src_d, art_points, art_d, reverse_delta, triangles, certificate = cache[cache_key]
            row['collision_triangles'] = triangles
            row['surface_certificate'] = certificate
            ref_states, src_states = ref.render_states(geometry), src.render_states(src_geom)
            variants = {}
            for state in sorted(set(ref_states) | set(src_states)):
                a = ref_states.get(state, ref_states['model'])
                b = src_states.get(state, src_states['model'])
                if a == (ref_model,) and b == (src_model,):
                    variants[state] = certificate
                    continue
                if any(path is None for path in a + b):
                    variants[state] = dict(status='unmeasured', complete=False, reason='missing configured render-state model')
                    continue
                variant_key = (tuple(map(str, a)), tuple(map(str, b)), geometry)
                if variant_key not in cache:
                    r, s = surface_union(variant_key[0]), surface_union(variant_key[1])
                    if collision_points:
                        variant = continuous_alignment(ref, geometry, r, s, tolerance, max_cells, collision_work=False)
                    elif r.triangles == s.triangles:
                        variant = dict(status='pass', complete=True, method='identical_indexed_surface', maximum_observed_m=0.)
                    else:
                        aa, bb = art_certificate(r, s, tolerance, max_cells), art_certificate(s, r, tolerance, max_cells)
                        variant = dict(status='pass' if aa['status'] == bb['status'] == 'pass' else 'unmeasured',
                                       complete=aa['status'] == bb['status'] == 'pass', method='visual_only_bidirectional_hausdorff',
                                       reference_to_source=aa, source_to_reference=bb)
                    cache[variant_key] = variant
                variants[state] = cache[variant_key]
            row['render_state_certificates'] = variants
            opacity = {}
            for state in sorted(set(ref_states) | set(src_states)):
                a = ref_states.get(state, ref_states['model'])
                b = src_states.get(state, src_states['model'])
                if any(path is None for path in a + b):
                    opacity[state] = dict(status='unmeasured', error='missing render-state model')
                    continue
                a_signatures = [opacity_signature(path, ref) for path in a]
                b_signatures = [opacity_signature(path, src) for path in b]
                # Fully opaque material count and naming do not change the
                # surface. For cutouts, preserve each material/UV dependency.
                a_signatures = [value for value in a_signatures if value is not None]
                b_signatures = [value for value in b_signatures if value is not None]
                equal = a_signatures == b_signatures
                opacity[state] = dict(status='pass' if equal else 'unmeasured',
                                      reference_sha256=a_signatures, source_sha256=b_signatures,
                                      reason='identical opacity dependencies' if equal else 'different opacity dependencies require visible-surface measurement')
            row['opacity_certificates'] = opacity
            row['reference_opacity_basis'] = 'PS2 base textures only'
            row['source_opacity_basis'] = 'all present base/enhanced texture tiers'
            row['lod_switch_identical'] = ref.lod_switch(inst, geometry) == src.lod_switch(src_inst, src_geom)
            row['render_state_world_witnesses'] = {
                state: {direction: list(placed(value['witness']['point'], inst))
                        for direction, value in variant.items()
                        if isinstance(value, dict) and (value.get('witness') or {}).get('point') is not None}
                for state, variant in variants.items()}
            variant_status = ('offender' if any(v['status'] == 'offender' for v in variants.values()) else 'unmeasured'
                              if any(v['status'] == 'unmeasured' for v in variants.values())
                              or any(v['status'] != 'pass' for v in opacity.values()) else 'pass')
            row['local_object_status'] = variant_status
            if (any(b - a > tolerance for a, b in zip(ref_d, src_d)) or any(value > tolerance for value in reverse_delta)):
                row['local_object_status'] = 'offender'
            row['local_object_status_kind'] = 'all-authored-body per-model sample flag; inactive layers and scene partition are not physical-gap proof'
            from render_scene_surface import camera_visibility_identical, scene_certificates
            row['camera_visibility_identical'] = camera_visibility_identical(ref, src, identity)
            scene_proof = scene_certificates(ref, src, identity, geometry, src_geom, variants, tolerance, max_cells)
            row['scene_surface_certificates'] = scene_proof
            row['measurement_seconds'] = time.perf_counter() - started
            if progress and row['measurement_seconds'] >= .1:
                progress(f'{label} {key} object {ordinal}:{index} {geometry} {row["measurement_seconds"]:.3f}s')
            row['scene_world_witnesses'] = {state: {direction: value['witness']['point']
                                                   for direction, value in proof.items()
                                                   if isinstance(value, dict) and (value.get('witness') or {}).get('point') is not None}
                                            for state, proof in scene_proof.items()
                                            if proof.get('method') == 'whole_scene_bidirectional_hausdorff'}
            # Per-model shape differences can be an assembly difference. Only
            # a complete scene-surface certificate accepts a different donor;
            # a failed conservative artwork bound is unmeasured, not a physical
            # offender inferred from a model partition.
            variant_status = ('pass' if all(value.get('status') == 'pass' for value in scene_proof.values())
                              and all(value.get('status') == 'pass' for value in opacity.values())
                              and row['lod_switch_identical'] else 'unmeasured')
            if not collision_points:
                row.update(status='baseline_visual_only' if variant_status == 'pass' else 'unmeasured',
                           baseline='PS2 and source both have render art without a geometry collision surface')
                if row['status'] == 'unmeasured':
                    row['error'] = 'different visual-only art lacks a continuous correspondence certificate'
                continue
            chosen = [i for i, p in enumerate(collision_points) if ref.in_play(placed(p, inst))]
            row['surface_samples'] = len(collision_points)
            row['playable_samples'] = len(chosen)
            row['diagnostic_maxima_kind'] = 'sampled lower bounds on maxima; not complete PS2 residual maxima'
            row['diagnostic_percentiles_kind'] = 'percentiles of deterministic diagnostic samples only'
            row['diagnostic_collision_basis'] = 'all authored bodies; includes inactive layers, not an actual player-contact subset'
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
            row['status'] = variant_status
            row['coverage'] = 'complete object, including outside playable bounds and surfaces visible from play'
            if row['status'] == 'unmeasured':
                row['error'] = 'continuous surface certificate unresolved; see forward/reverse reasons'
            for direction in ('forward', 'reverse'):
                witness = certificate.get(direction, {}).get('witness')
                if witness and witness.get('point') is not None:
                    row.setdefault('world_witnesses', {})[direction] = list(placed(witness['point'], inst))
        except (ValueError, OSError, KeyError, IndexError, TypeError) as exc:
            row.update(status='unmeasured', error=str(exc))
            if str(exc).startswith('invalid collision primitive dimensions') and row.get('geometry') in ref.collisions:
                _folder, collision = ref.collisions[row['geometry']]
                invalid = []
                for primitive in collision.get('prims', []):
                    used = {'box': (0, 1, 2), 'sphere': (0,), 'cylinder': (0, 1)}.get(primitive.get('kind'), ())
                    dims = primitive.get('dims', [])
                    if len(dims) != 3 or any(not math.isfinite(dims[i]) or dims[i] < 0 for i in used):
                        invalid.append(dict(name=primitive.get('name'), kind=primitive.get('kind'), dims=dims,
                                            used_dimensions=list(used), mask=primitive.get('mask'),
                                            world_location=list(placed(primitive['translation'], inst))))
                row['invalid_collision_primitives'] = invalid
            if str(exc).startswith('unresolved placed class'):
                try:
                    baseline = unresolved_class_baseline(ref, src, identity)
                    if baseline is not None:
                        row.update(baseline)
                        row.pop('error', None)
                except (ValueError, OSError, KeyError, IndexError, TypeError) as baseline_error:
                    row['baseline_error'] = str(baseline_error)
    extras = sorted(set(src.instances) - set(ref.instances))
    summary = {status: sum(r['status'] == status for r in rows) for status in sorted({r['status'] for r in rows})}
    result = dict(source=label, map=key, placements=len(rows), extra_source_placements=len(extras), counts=summary, objects=rows)
    for name in ('terrain', 'attachment'):
        try:
            if name == 'terrain':
                from render_terrain_check import terrain_map
                receipt = terrain_map(reference, other, key)
            else:
                from render_attachment_check import attachment_map
                receipt = attachment_map(reference, other, key)
        except (ImportError, ValueError, OSError, KeyError, IndexError, TypeError) as exc:
            receipt = dict(complete=False, status='unmeasured', error=str(exc))
        result[name] = receipt
    return result


def certificate_problems(certificate, tolerance):
    if not isinstance(certificate, dict) or certificate.get('status') != 'pass' or certificate.get('complete') is not True:
        return ['missing complete continuous surface certificate']
    method = certificate.get('method')
    if method == 'identical_indexed_surface':
        value = certificate.get('maximum_observed_m')
        return [] if type(value) in (int, float) and value == 0. else ['invalid exact-surface certificate']
    if method in ('visual_only_bidirectional_hausdorff', 'placed_object_bidirectional_hausdorff', 'whole_scene_bidirectional_hausdorff'):
        stretch = certificate.get('stretch_upper', 1.)
        if not isinstance(stretch, (int, float)) or not math.isfinite(stretch) or stretch < 1:
            return ['invalid placed-surface stretch bound']
        if method == 'whole_scene_bidirectional_hausdorff':
            lifetime = certificate.get('lifetime')
            if (not isinstance(lifetime, dict) or set(lifetime) != {'reference', 'source'}
                    or any(not isinstance(value, dict) or value.get('eligible') is not True
                           or not isinstance(value.get('class_inputs_sha256'), str)
                           or len(value['class_inputs_sha256']) != 64
                           or type(value.get('class_files')) is not int or value['class_files'] < 1
                           for value in lifetime.values())):
                return ['scene borrowing has no complete class lifetime proof']
        return [problem for key in ('reference_to_source', 'source_to_reference')
                for problem in certificate_problems(certificate.get(key), tolerance / stretch)]
    if 'forward' in certificate and 'reverse' in certificate:
        return [problem for key in ('forward', 'reverse') for problem in certificate_problems(certificate[key], tolerance)]
    allowed = ('continuous_directed_hausdorff', 'reference_to_source_hausdorff_sufficient_bound', 'source_to_reference_hausdorff_sufficient_bound',
               'pointwise_collision_residual_difference',
               'min_art_distance_and_collision_distance_sufficient_bound', 'reference_art_distance_sufficient_bound')
    upper = certificate.get('certified_upper_m')
    if method not in allowed or type(upper) not in (int, float) or not math.isfinite(upper) or upper > tolerance:
        return ['invalid continuous residual upper bound']
    if (method != 'pointwise_collision_residual_difference' and upper < 0
            or type(certificate.get('cells')) is not int or certificate['cells'] <= 0
            or type(certificate.get('certified_patches')) is not int or certificate['certified_patches'] <= 0):
        return ['continuous certificate has no covered patches']
    return []


def report_problems(report, reference, cases, expected_tolerance=DEFAULT_TOLERANCE_M):
    """Validate a completed gate receipt independently of the helper exit code."""
    errors = []
    if not isinstance(report, dict) or report.get('schema') != SCHEMA or report.get('complete') is not True:
        return ['missing complete schema-2 report']
    tolerance = report.get('tolerance_m')
    if (report.get('method') != METHOD or not isinstance(tolerance, (int, float)) or not math.isfinite(tolerance)
            or not 0 <= tolerance <= expected_tolerance or not report.get('margin_reason')):
        return ['invalid continuous method or alignment margin']
    if not isinstance(report.get('reference'), str) or Path(report['reference']).resolve() != Path(reference).resolve():
        errors.append('reference assets do not match requested input')
    rows = report.get('maps')
    if not isinstance(rows, list) or not rows:
        return errors + ['report has no source/map rows']
    names = [case.split('=', 1)[0] for case in cases]
    if len(set(names)) != len(names) or any(not name for name in names):
        return errors + ['invalid or duplicate requested source names']
    requested_paths = {name: str(Path(folder).resolve()) for name, folder in (case.split('=', 1) for case in cases)}
    keys = set(REQUIRED_MAPS) | {p.parent.name for root in (Path(reference), *(Path(folder) for folder in requested_paths.values()))
                               for p in root.glob('*/manifest.ron')}
    expected = {(name, key) for name in names for key in keys}
    if report.get('source_paths') != requested_paths:
        errors.append('source asset paths do not match requested inputs')
    actual = []
    statuses = {'pass', 'offender', 'unmeasured', 'baseline_visual_only', 'baseline_no_render_geometry', 'baseline_collision_only', 'baseline_runtime_skip'}
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
        for name in ('terrain', 'attachment'):
            receipt = row.get(name)
            try:
                if name == 'terrain':
                    from render_terrain_check import terrain_problems
                    problems = terrain_problems(receipt)
                else:
                    from render_attachment_check import attachment_problems
                    problems = attachment_problems(receipt)
                errors.extend(f"{row['source']} {row['map']} {name}: {problem}" for problem in problems)
            except (ImportError, ValueError, TypeError, KeyError) as exc:
                errors.append(f'{name} receipt validator unavailable/invalid: {exc}')
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
            if status in ('pass', 'baseline_visual_only'):
                if obj.get('collision_identical') is not True:
                    errors.append('corrected collision identity was not verified')
                if obj.get('class_collision_identical') is not True:
                    errors.append('resolved class collision-layer/motion policy identity was not verified')
                variants = obj.get('scene_surface_certificates')
                if not isinstance(variants, dict) or not {'model', 'lowd'} <= set(variants):
                    errors.append('missing model/LOWD scene-surface certificates')
                else:
                    errors.extend(problem for cert in variants.values() for problem in certificate_problems(cert, tolerance))
                opacity = obj.get('opacity_certificates')
                if (not isinstance(opacity, dict) or not isinstance(variants, dict) or set(opacity) != set(variants)
                        or any(not isinstance(v, dict) or v.get('status') != 'pass'
                               or v.get('reference_sha256') != v.get('source_sha256') for v in opacity.values())):
                    errors.append('missing/unmeasured opacity dependency certificates')
                if obj.get('lod_switch_identical') is not True:
                    errors.append('unmeasured LOD switch geometry/range correspondence')
                if obj.get('camera_visibility_identical') is not True:
                    errors.append('unmeasured camera-fade geometry correspondence')
            elif status in ('baseline_no_render_geometry', 'baseline_collision_only', 'baseline_runtime_skip') and not obj.get('baseline'):
                errors.append('no-render baseline was not identified')
            if status in ('baseline_no_render_geometry', 'baseline_collision_only', 'baseline_runtime_skip'):
                if obj.get('class_collision_identical') is not True:
                    errors.append('no-render baseline lacks resolved policy identity')
                if status == 'baseline_runtime_skip':
                    proof = obj.get('raw_consumer_proof')
                    if (obj.get('resolved_class_absent_both') is not True or obj.get('runtime_consumer_skip_proven') is not True
                            or obj.get('layout_inputs_identical') is not True
                            or not isinstance(proof, dict) or set(proof) != {'reference', 'source'}):
                        errors.append('unproved runtime skip')
                    else:
                        for side in proof.values():
                            if (not isinstance(side, dict) or side.get('skip') is not True
                                    or side.get('engine_base') not in (None, 'prop', 'building', 'commandpost')
                                    or type(side.get('class_files')) is not int or side['class_files'] <= 0
                                    or not isinstance(side.get('class_inputs_sha256'), str) or len(side['class_inputs_sha256']) != 64):
                                errors.append('invalid raw runtime-consumer skip evidence')
                elif obj.get('render_absent_both'):
                    if (obj.get('raw_class_inputs_identical') is not True or not isinstance(obj.get('raw_class_inputs_sha256'), str)
                            or len(obj['raw_class_inputs_sha256']) != 64 or not isinstance(obj.get('raw_class_files'), int) or obj['raw_class_files'] <= 0):
                        errors.append('missing raw runtime-consumer inputs for absent direct art/class')
                if status == 'baseline_collision_only' and (obj.get('render_absent_both') is not True or obj.get('collision_identical') is not True):
                    errors.append('invisible collision baseline was not proved')
        if measured != {k: v for k, v in counts.items() if v}:
            errors.append('placed-object counts do not match results')
        if not (counts.get('pass') or counts.get('offender')):
            errors.append('map has no measured collision-surface results')
    if len(actual) != len(set(actual)):
        errors.append('duplicate source/map rows')
    if set(actual) != expected:
        errors.append(f'source/map coverage differs: {len(expected - set(actual))} missing, {len(set(actual) - expected)} extra')
    return errors


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--reference', type=Path, required=True)
    p.add_argument('--case', action='append', required=True, metavar='NAME=ASSETS')
    p.add_argument('--map', action='append', help='scoped diagnostic maps; incomplete catalog runs cannot return acceptance success')
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--table', type=Path)
    p.add_argument('--tolerance', type=float, default=DEFAULT_TOLERANCE_M,
                   help='extra gap relative to PS2 art in metres (default .01; up to .1 for historical diagnostics, not build acceptance)')
    p.add_argument('--samples', type=int, default=256, help='diagnostic samples only; acceptance always requires continuous certificates')
    p.add_argument('--max-cells', type=int, default=DEFAULT_MAX_CELLS, help='continuous work budget per geometry/direction; exhausted budgets fail unmeasured')
    args = p.parse_args()
    if not math.isfinite(args.tolerance) or not 0 <= args.tolerance <= MAX_TOLERANCE_M or args.samples < 4 or args.max_cells < 1:
        p.error('tolerance must be finite and between 0 and .1; samples must be >=4; max-cells must be >=1')
    try:
        cases = [(name, Path(folder)) for name, folder in (s.split('=', 1) for s in args.case)]
    except ValueError:
        p.error('each --case must be NAME=ASSETS')
    if any(not name for name, _folder in cases) or len({name for name, _folder in cases}) != len(cases):
        p.error('case names must be nonempty and unique')
    full_keys = set(REQUIRED_MAPS) | {path.parent.name for root in (args.reference, *(folder for _name, folder in cases))
                                    for path in root.glob('*/manifest.ron')}
    keys = args.map or sorted(full_keys)
    acceptance_eligible = set(keys) == full_keys and args.tolerance <= DEFAULT_TOLERANCE_M
    if not keys:
        p.error('reference has no maps')
    rows = []
    for name, folder in cases:
        for key in keys:
            try:
                row = compare_map(args.reference, folder, key, name, args.tolerance, args.samples, args.max_cells,
                                  progress=lambda message: print(message, flush=True))
            except (ValueError, OSError, KeyError, IndexError) as exc:
                row = dict(source=name, map=key, error=str(exc), counts={'unmeasured': 1}, placements=0, extra_source_placements=0)
            rows.append(row)
            print(name, key, row['counts'], flush=True)
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(json.dumps(dict(schema=SCHEMA, complete=False, reference=str(args.reference),
                                    source_paths={name: str(folder.resolve()) for name, folder in cases}, acceptance_eligible=acceptance_eligible,
                                    tolerance_m=args.tolerance, margin_reason=MARGIN_REASON, diagnostic_samples_per_mesh=args.samples,
                                    method=METHOD, maps=rows), indent=2) + '\n')
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(dict(schema=SCHEMA, complete=True, reference=str(args.reference), tolerance_m=args.tolerance,
                                      source_paths={name: str(folder.resolve()) for name, folder in cases}, acceptance_eligible=acceptance_eligible,
                                      margin_reason=MARGIN_REASON,
                                      diagnostic_samples_per_mesh=args.samples, method=METHOD, maps=rows), indent=2) + '\n')
    if args.table:
        lines = ['source,map,placements,pass,offender,unmeasured,baseline_visual_only,baseline_no_render_geometry,baseline_collision_only,baseline_runtime_skip,terrain,attachment']
        for r in rows:
            c = r['counts']
            lines.append(','.join(str(v) for v in (r['source'], r['map'], r['placements'], c.get('pass', 0), c.get('offender', 0),
                         c.get('unmeasured', 0), c.get('baseline_visual_only', 0), c.get('baseline_no_render_geometry', 0), c.get('baseline_collision_only', 0), c.get('baseline_runtime_skip', 0),
                         r.get('terrain', {}).get('status', 'unmeasured'), r.get('attachment', {}).get('status', 'unmeasured'))))
        args.table.parent.mkdir(parents=True, exist_ok=True)
        args.table.write_text('\n'.join(lines) + '\n')
    return int(not acceptance_eligible or any(r.get('error') or r['extra_source_placements'] or r['counts'].get('offender') or r['counts'].get('unmeasured')
                   or any(r.get(name, {}).get('complete') is not True or r.get(name, {}).get('status') != 'pass'
                          for name in ('terrain', 'attachment')) for r in rows))


if __name__ == '__main__':
    raise SystemExit(main())
