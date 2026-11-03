#!/usr/bin/env python3
"""Exact-arithmetic local foot bounds for skins with the same joint poses.

A point within epsilon of a convex combination of another foot's vertices
bounds its support height in EVERY direction by epsilon. With one full joint
influence per vertex, the same bound holds after any shared rigid joint pose.
Checking every indexed vertex owned by those joints in both directions bounds
their support height, including complete triangles joining those vertices.
This is one model-state pair. Callers must cover all possible HIGH/LOW pairs;
passing this helper does not certify a complete soldier or its LOD selection.
The bound uses exact arithmetic and exact SLERP. Runtime palette error and
the world/GPU transform's rounding remain explicit caller obligations.
"""
from functools import lru_cache
from fractions import Fraction
import argparse
import hashlib
from itertools import permutations
import json
import math
from pathlib import Path
import struct

from world_fingerprint import digest, read_glb, read_ron
from render_collision_check import Surface, art_certificate, triangle_distance_upper

MARGIN_M = .01
SOLE_BAND_M = .10


@lru_cache(maxsize=16)
def _load(path, modified, size):
    del modified, size
    return read_glb(Path(path))


def _read(path):
    stat = path.stat()
    return _load(str(path.resolve()), stat.st_mtime_ns, stat.st_size)


def _pose_path(path):
    return path.with_name(path.name.replace('.low.glb', '.glb'))


def _companion(path):
    path = _pose_path(path)
    return path.with_name(path.stem + '.render-clips.glb')


def _visual_animations(path, doc, accessor):
    animations = [(animation, accessor) for animation in doc.get('animations', [])]
    companion = _companion(path)
    if not companion.is_file():
        return animations
    visual, read = _read(companion)
    joints, parents, names = _nodes(doc)
    vj, vp, vn = _nodes(visual)
    def skeleton(data, js, ps):
        return [[data['nodes'][n].get('name'),
                 js.index(ps[n]) if ps.get(n) in js else None,
                 data['nodes'][n].get('translation', [0, 0, 0]),
                 data['nodes'][n].get('rotation', [0, 0, 0, 1]),
                 data['nodes'][n].get('scale', [1, 1, 1]),
                 data['nodes'][n].get('matrix')] for n in js]
    if visual.get('meshes') or joints != vj or names != vn or skeleton(doc, joints, parents) != skeleton(visual, vj, vp):
        raise ValueError('visual clip companion changes the body mesh or skeleton')
    replacements, seen = visual.get('animations', []), set()
    if not replacements:
        raise ValueError('empty visual clip companion')
    for animation in replacements:
        name = animation.get('name', '')
        found = [i for i, (original, _read) in enumerate(animations) if original.get('name') == name]
        if len(found) != 1 or name.lower() in seen:
            raise ValueError('ambiguous visual clip replacement')
        index = found[0]
        original, original_read = animations[index]
        # Our companion copies a whole clip with the same sampler layout.
        # Equal channel timing is stronger than the runtime's fps/frame check.
        before = [original_read(s['input']) for s in original['samplers']]
        after = [read(s['input']) for s in animation['samplers']]
        if before != after:
            raise ValueError('visual clip companion changes timing')
        seen.add(name.lower())
        animations[index] = (animation, read)
    return animations


def _fraction_f32(value):
    return Fraction(struct.unpack('<f', struct.pack('<f', value))[0])


@lru_cache(maxsize=32768)
def _norm_error(q):
    # Products of two binary32 significands fit exactly in binary64.
    # fsum rounds their exact sum once; enclose that final rounding.
    values = [struct.unpack('<f', struct.pack('<f', x))[0] for x in q]
    squared = math.fsum(value * value for value in values)
    return max(abs(math.nextafter(squared, -math.inf) - 1),
               abs(math.nextafter(squared, math.inf) - 1))


def _rotation(q):
    x, y, z, w = map(_fraction_f32, q)
    return [[1 - 2 * (y*y + z*z), 2 * (x*y - z*w), 2 * (x*z + y*w)],
            [2 * (x*y + z*w), 1 - 2 * (x*x + z*z), 2 * (y*z - x*w)],
            [2 * (x*z - y*w), 2 * (y*z + x*w), 1 - 2 * (x*x + y*y)]]


def _multiply(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def _sqrt_upper(value):
    result = math.sqrt(float(value))
    while Fraction(result) * Fraction(result) < value:
        result = math.nextafter(result, math.inf)
    return result


def _inverse_frobenius(matrix):
    cofactors = []
    for i in range(3):
        row = []
        for j in range(3):
            rows = [n for n in range(3) if n != i]
            cols = [n for n in range(3) if n != j]
            minor = matrix[rows[0]][cols[0]] * matrix[rows[1]][cols[1]] - matrix[rows[0]][cols[1]] * matrix[rows[1]][cols[0]]
            row.append(minor * (-1 if (i + j) % 2 else 1))
        cofactors.append(row)
    determinant = sum(matrix[0][j] * cofactors[0][j] for j in range(3))
    if determinant == 0:
        raise ValueError('singular recomputed bind rotation')
    # Transposition in the adjugate leaves its Frobenius norm unchanged.
    return _sqrt_upper(sum(value * value for row in cofactors for value in row) / (determinant * determinant))


def _nodes(doc):
    if len(doc.get('skins', [])) != 1 or doc.get('images'):
        raise ValueError('unsupported skin/image load dependencies')
    skin = doc['skins'][0]
    joints = skin['joints']
    if not joints or len(joints) > 256 or len(set(joints)) != len(joints):
        raise ValueError('invalid skin joint table')
    if any(type(node) is not int or not 0 <= node < len(doc['nodes']) for node in joints):
        raise ValueError('invalid skin joint node')
    parents = {}
    for index, row in enumerate(doc['nodes']):
        for child in row.get('children', []):
            if type(child) is not int or not 0 <= child < len(doc['nodes']) or child in parents:
                raise ValueError('invalid/ambiguous node parent')
            parents[child] = index
    for first in range(len(doc['nodes'])):
        seen = set()
        node = first
        while node is not None:
            if node in seen:
                raise ValueError('skin node graph cycle')
            seen.add(node)
            node = parents.get(node)
    names = [doc['nodes'][node].get('name', '').lower() for node in joints]
    if len(set(names)) != len(names) or any(not name for name in names):
        raise ValueError('ambiguous skin joint names')
    for joint in joints:
        seen = set()
        node = joint
        while node in joints:
            if node in seen:
                raise ValueError('skin parent cycle')
            seen.add(node)
            node = parents.get(node)
    return joints, parents, names


def _parts(path, requested=None):
    doc, accessor = _read(path)
    joints, _parents, names = _nodes(doc)
    mesh_node = next(node for node in doc['nodes'] if 'skin' in node and 'mesh' in node)
    if mesh_node['skin'] != 0:
        raise ValueError('first skinned mesh does not use the first skin')
    raw = []
    for part in doc['meshes'][mesh_node['mesh']]['primitives']:
        if part.get('mode', 4) != 4:
            raise ValueError('unsupported skin primitive mode')
        attrs = part['attributes']
        descriptors = doc['accessors']
        if (descriptors[attrs['POSITION']]['componentType'] != 5126
                or descriptors[attrs['POSITION']]['type'] != 'VEC3'
                or descriptors[attrs['JOINTS_0']]['componentType'] not in (5121, 5123)
                or descriptors[attrs['JOINTS_0']]['type'] != 'VEC4'
                or descriptors[attrs['WEIGHTS_0']]['type'] != 'VEC4'):
            raise ValueError('unsupported skin attribute format')
        positions = accessor(attrs['POSITION'])
        weights = accessor(attrs['WEIGHTS_0'])
        bones = accessor(attrs['JOINTS_0'])
        scale = {5121: 255, 5123: 65535, 5126: 1}[doc['accessors'][attrs['WEIGHTS_0']]['componentType']]
        indices = [row[0] for row in accessor(part['indices'])]
        if not indices or len(indices) % 3 or any(not 0 <= i < len(positions) for i in indices):
            raise ValueError('invalid skin indices')
        if len(weights) != len(positions) or len(bones) != len(positions):
            raise ValueError('incomplete skin vertex streams')
        bound = []
        for point, js, ws in zip(positions, bones, weights):
            active = [i for i, w in enumerate(ws) if w != 0]
            if (len(point) != 3 or not all(math.isfinite(v) for v in point)
                    or len(active) != 1 or ws[active[0]] != scale):
                raise ValueError('support proof requires one full joint influence per vertex')
            joint = js[active[0]]
            if type(joint) is not int or not 0 <= joint < len(joints):
                raise ValueError('invalid skin joint index')
            bound.append(names[joint])
        material = doc.get('materials', [])[part['material']] if 'material' in part else {}
        if material.get('alphaMode') == 'MASK':
            # load_skinned_glb rejects the whole body, even when only an
            # unrelated upper-body part asks for the unsupported pass.
            raise ValueError('a MASK part prevents the entire skin from loading')
        raw.append((positions, bound, indices, material.get('alphaMode', 'OPAQUE')))
    used = [(positions[i], bound[i]) for positions, bound, indices, _mode in raw for i in set(indices)]
    if not used:
        raise ValueError('empty drawn skin')
    chosen = set(requested or ())
    if requested is None:
        chosen = {name for _point, name in used if '_foot' in name or '_toe' in name}
        if not chosen:
            floor = min(point[1] for point, _name in used)
            chosen = {name for point, name in used if point[1] <= floor + SOLE_BAND_M}
    groups, faces = {}, []
    for positions, bound, indices, mode in raw:
        for first in range(0, len(indices), 3):
            tri = indices[first:first + 3]
            if any(bound[i] in chosen for i in tri):
                if mode != 'OPAQUE':
                    raise ValueError('selected foot/support region has unproved blended coverage')
                faces.append(tuple((bound[i], tuple(positions[i])) for i in tri))
        for name in chosen:
            own = {i for i in indices if bound[i] == name}
            if not own:
                continue
            if mode != 'OPAQUE':
                raise ValueError('selected foot/support group has unproved blended coverage')
            group = groups.setdefault(name, {'vertices': set(), 'triangles': []})
            group['vertices'].update(tuple(positions[i]) for i in own)
            for first in range(0, len(indices), 3):
                triangle = indices[first:first + 3]
                if all(bound[i] == name for i in triangle):
                    group['triangles'].append(tuple(tuple(positions[i]) for i in triangle))
    return groups, faces


def _pose_dependencies(path, chosen):
    inputs = [path, _pose_path(path)]
    companion = _companion(path)
    if companion.is_file():
        inputs.append(companion)
    base = _pose_path(path).with_suffix('')
    inputs.extend(Path(str(base) + suffix) for suffix in ('.sets.ron', '.anims.ron', '.motion.ron'))
    signature = tuple((str(p.resolve()), p.stat().st_mtime_ns, p.stat().st_size) for p in inputs)
    return _pose_dependencies_cached(str(path), signature, tuple(sorted(chosen)))


@lru_cache(maxsize=256)
def _pose_dependencies_cached(path, signature, chosen):
    del signature
    path = Path(path)
    # A LOW mesh is driven by the HIGH body's palette, not its empty clips.
    doc, accessor = _read(_pose_path(path))
    joints, parents, names = _nodes(doc)
    if path != _pose_path(path):
        low_doc, _accessor = _read(path)
        low_joints, low_parents, low_names = _nodes(low_doc)
        def binds(data, js, ps):
            return [[data['nodes'][n].get('translation', [0, 0, 0]),
                     data['nodes'][n].get('rotation', [0, 0, 0, 1]),
                     data['nodes'][n].get('scale', [1, 1, 1]), data['nodes'][n].get('matrix'),
                     js.index(ps[n]) if ps.get(n) in js else None] for n in js]
        if low_names != names or binds(low_doc, low_joints, low_parents) != binds(doc, joints, parents):
            raise ValueError('LOW companion does not share the HIGH bind rig')
    selected = set()
    depth = 0
    for name in chosen:
        if name not in names:
            raise ValueError('support joint absent from the high rig')
        node = joints[names.index(name)]
        chain = set()
        while node is not None:
            if node in chain:
                raise ValueError('skin parent cycle')
            chain.add(node)
            selected.add(node)
            node = parents.get(node)
        depth = max(depth, len(chain))
    if depth > 32:
        raise ValueError('skin parent chain exceeds stretch proof')
    identity = []
    error = 0.
    for node in sorted(selected):
        value = doc['nodes'][node]
        if 'matrix' in value or value.get('scale', [1, 1, 1]) != [1, 1, 1]:
            raise ValueError('scaled/matrix skin node requires its own stretch proof')
        q = value.get('rotation', [0, 0, 0, 1])
        error = max(error, _norm_error(tuple(q)))
        identity.append([value.get('name'), doc['nodes'][parents[node]].get('name') if node in parents else None,
                         value.get('translation', [0, 0, 0]), q])
    animations = []
    for animation, animation_read in _visual_animations(path, doc, accessor):
        timing = [animation_read(sampler['input']) for sampler in animation['samplers']]
        tracks = []
        for channel in animation['channels']:
            node, kind = channel['target']['node'], channel['target']['path']
            if node not in selected:
                continue
            sampler = animation['samplers'][channel['sampler']]
            output = animation_read(sampler['output'])
            if kind == 'rotation':
                for q in output:
                    error = max(error, _norm_error(tuple(q)))
            elif kind != 'translation':
                raise ValueError('non-rigid support animation channel')
            tracks.append([doc['nodes'][node].get('name'), kind, sampler.get('interpolation', 'LINEAR'),
                           animation_read(sampler['input']), output])
        # The loader derives frame count/fps in channel order, including
        # channels outside the foot mask. Keep their routing and timing,
        # while unrelated upper-body key values need not be identical.
        animations.append([animation.get('name'), animation.get('extras'),
                           animation['channels'], timing, tracks])
    if not math.isfinite(error) or error > 1e-5:
        raise ValueError('support rotation norm exceeds stretch proof')
    bind = {}
    def global_bind(node):
        if node not in bind:
            local = _rotation(doc['nodes'][node].get('rotation', [0, 0, 0, 1]))
            parent = parents.get(node)
            bind[node] = _multiply(global_bind(parent), local) if parent in joints else local
        return bind[node]
    inverse = {name: _inverse_frobenius(global_bind(joints[names.index(name)])) for name in chosen}
    # For exact shortest-path SLERP, squared quaternion norm error cannot
    # exceed the endpoints' error. The rotation matrix's operator norm is
    # <=1+4*error. Products along the complete ancestor chain bound G_j.
    # This does NOT assert that glam's acos/sin approximations or f32
    # arithmetic are exact; the caller still owes those error bounds.
    rotation_envelope = math.nextafter(float((1 + 4 * Fraction(error)) ** depth), math.inf)
    base = _pose_path(path).with_suffix('')
    controls = []
    for suffix in ('.sets.ron', '.anims.ron', '.motion.ron'):
        filename = Path(str(base) + suffix)
        controls.append([suffix, read_ron(filename)])
    return digest([identity, animations, controls]), depth, float(error), inverse, rotation_envelope


def _vertex_upper(point, target):
    # Every candidate triangle is inside the convex hull of target vertices.
    # A nearest-face query only picks a candidate; the outward upper bound is
    # re-evaluated on that face before it can certify anything.
    nearest = min(target['vertices'], key=lambda other: math.dist(point, other))
    vertex = triangle_distance_upper(point, (nearest, nearest, nearest))
    if target['surface'] is not None:
        _distance, _closest, triangle = target['surface'].closest_triangle(point)
        return min(vertex, triangle_distance_upper(point, triangle))
    return vertex


def _point_upper(a, b):
    return triangle_distance_upper(a, (b, b, b))


def _surface_pair(a, b, faces_a, faces_b, operators, margin):
    """Every triangle, including triangles crossing a joint boundary.

    A pure-joint patch moves through one common affine matrix. A mixed patch
    needs a target triangle with corresponding joint roles at its corners.
    Equal barycentric coordinates then map every point of the source triangle
    into that target triangle at every pose. Convexity bounds its error by
    the largest transformed corner error; no rest-pose-only inference is used.
    """
    records = []
    for name in sorted(a):
        reference, source = a[name]['surface'], b[name]['surface']
        if reference is None and source is None:
            continue
        if reference is None or source is None:
            return dict(complete=False, reason='pure-joint surface ownership differs', groups=records)
        local_margin = margin / operators[name]
        forward = art_certificate(reference, source, local_margin, 100000)
        reverse = art_certificate(source, reference, local_margin, 100000)
        records.append(dict(name=name, forward=forward, reverse=reverse))
        if forward['status'] != 'pass' or reverse['status'] != 'pass':
            return dict(complete=False, reason='pure-joint surface bound unresolved', groups=records)
    mixed = [[face for face in faces if len({name for name, _point in face}) > 1]
             for faces in (faces_a, faces_b)]
    bridge_upper, checked = 0., 0
    for domain, target in (mixed, mixed[::-1]):
        for face in domain:
            key = sorted(name for name, _point in face)
            best = math.inf
            for candidate in target:
                if sorted(name for name, _point in candidate) != key:
                    continue
                for order in permutations(range(3)):
                    if any(face[i][0] != candidate[order[i]][0] for i in range(3)):
                        continue
                    bound = max(math.nextafter(operators[face[i][0]] * _point_upper(face[i][1], candidate[order[i]][1]), math.inf)
                                for i in range(3))
                    best = min(best, bound)
            checked += 1
            if best > margin:
                return dict(complete=False, reason='mixed-joint triangle lacks an all-pose correspondence',
                            groups=records, mixed_triangles_checked=checked,
                            witness=[list(point) for _name, point in face])
            bridge_upper = max(bridge_upper, best)
    upper = max([bridge_upper] + [math.nextafter(operators[row['name']] * row[direction]['certified_upper_m'], math.inf)
                                  for row in records for direction in ('forward', 'reverse')])
    return dict(complete=True, upper_m=upper, groups=records, mixed_triangles_checked=checked)


def support_pair(reference, source, margin=MARGIN_M):
    result = {'schema': 1, 'method': 'all_pose_joint_support', 'scope': 'one model-state pair',
              'arithmetic': 'exact bind construction and exact SLERP; actor-local coordinates',
              'margin_m': margin, 'status': 'unmeasured', 'complete': False, 'runtime_complete': False, 'groups': []}
    try:
        if type(margin) not in (int, float) or not math.isfinite(margin) or not 0 < margin <= MARGIN_M:
            raise ValueError('invalid support margin')
        reference, source = Path(reference), Path(source)
        result['reference_sha256'] = hashlib.sha256(reference.read_bytes()).hexdigest()
        result['source_sha256'] = hashlib.sha256(source.read_bytes()).hexdigest()
        a, faces_a = _parts(reference)
        b, faces_b = _parts(source)
        if set(a) != set(b) or not a:
            raise ValueError('support joint ownership differs; shared-pose proof cannot compare these states')
        roles = set(a) | {name for face in faces_a + faces_b for name, _point in face}
        pa, depth, error_a, inverse_a, envelope_a = _pose_dependencies(reference, roles)
        pb, _depth_b, error_b, inverse_b, envelope_b = _pose_dependencies(source, roles)
        result.update(pose_inputs_identical=pa == pb, reference_pose_sha256=pa, source_pose_sha256=pb,
                      maximum_ancestor_depth=depth, maximum_key_quaternion_norm_error=max(error_a, error_b))
        if pa != pb:
            raise ValueError('support ancestor pose/controller inputs differ')
        for groups in (a, b):
            for group in groups.values():
                vertices = [point for tri in group['triangles'] for point in tri]
                group['surface'] = Surface(vertices, list(range(len(vertices)))) if vertices else None
        operators = {name: math.nextafter(max(inverse_a[name], inverse_b[name]) * max(envelope_a, envelope_b), math.inf)
                     for name in roles}
        for name in sorted(a):
            forward = max((_vertex_upper(point, b[name]), point) for point in a[name]['vertices'])
            reverse = max((_vertex_upper(point, a[name]), point) for point in b[name]['vertices'])
            bind_upper = max(inverse_a[name], inverse_b[name])
            operator = operators[name]
            upper = math.nextafter(operator * max(forward[0], reverse[0]), math.inf)
            result['groups'].append({'name': name, 'reference_vertices': len(a[name]['vertices']),
                                     'source_vertices': len(b[name]['vertices']), 'support_upper_m': upper,
                                     'inverse_bind_frobenius_upper': bind_upper,
                                     'exact_slerp_rotation_envelope_upper': max(envelope_a, envelope_b),
                                     'joint_operator_upper': operator,
                                     'reference_witness': forward[1], 'source_witness': reverse[1]})
        result['vertex_support_upper_m'] = max(row['support_upper_m'] for row in result['groups'])
        surface = _surface_pair(a, b, faces_a, faces_b, operators, margin)
        result['surface_certificate'] = surface
        result['support_upper_m'] = min(result['vertex_support_upper_m'], surface['upper_m']) if surface['complete'] else result['vertex_support_upper_m']
        result['complete'] = surface['complete'] and result['support_upper_m'] <= margin
        result['status'] = 'pass' if result['complete'] else 'unmeasured'
        if not result['complete']:
            result['reason'] = surface.get('reason', 'convex support bound cannot certify the requested margin')
        # Soldier body_draws always submits OPAQUE parts. A missing texture
        # becomes Renderer::tex_bg's white fallback, not a skipped world part.
        result['draw_coverage'] = 'opaque skinned parts; optional texture uses the white fallback'
        result['requires'] = ['same actor/controller input', 'all selected model-state pairs',
                              'successful actual body loader and draw submission',
                              'runtime palette/inverse-bind f32 and approximate-SLERP error bound',
                              'world/GPU transform roundoff bound']
    except (ValueError, KeyError, IndexError, TypeError, OSError, StopIteration, OverflowError, struct.error) as error:
        result['reason'] = str(error)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    receipt = support_pair(args.reference, args.source)
    args.out.write_text(json.dumps(receipt, indent=2, allow_nan=False) + '\n')
    print(json.dumps({key: receipt.get(key) for key in ('scope', 'status', 'support_upper_m', 'reason')}))
    return int(not receipt['complete'])


if __name__ == '__main__':
    raise SystemExit(main())
