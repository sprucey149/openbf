"""Placed render-surface union for strict alignment certificates.

Only immutable neighboring objects can supply coverage for a model assembled
differently by another source. Mode layers never cover one another. A neighbor
with different LOD surfaces or a moving rig is not borrowed, because that art
could disappear at another camera position or animation pose. Every such object
still gets its own placed-object measurement in the caller.
"""
from functools import lru_cache
import heapq
import math
from pathlib import Path

from render_collision_check import Surface, art_certificate, bounds, box_distance2, dot, opacity_signature, placed, surface_union
from world_fingerprint import digest, glb_geometry, read_glb
from render_model_dependencies import model_descriptor


@lru_cache(maxsize=512)
def _camera_mesh(path, modified, size):
    del modified, size
    # CameraBlockers uses ALL loaded positions for its box, including spare
    # unindexed vertices, and the complete indexed surface for sight sweeps.
    positions, indices = glb_geometry(Path(path))
    doc, _accessor = read_glb(Path(path))
    ranges = [(doc['accessors'][part['attributes']['POSITION']]['count'],
               doc['accessors'][part['indices']]['count'] if 'indices' in part else None)
              for mesh in doc.get('meshes', []) for part in mesh.get('primitives', [])]
    flags = []
    for mesh in doc.get('meshes', []):
        extras = mesh.get('extras') or {}
        flag = extras.get('contact_surface', False)
        if type(flag) is not bool:
            raise ValueError('contact_surface draw policy is not a bool')
        flags.append(flag)
    return bounds(positions), digest([positions, indices, ranges, flags]), any(flags)


def camera_descriptor(scene, instance):
    """Conservative camera-fade dependency of a placed object's primary model."""
    _geometry, path = scene.resolve(instance)
    if path is None:
        raise ValueError('camera blocker primary model missing')
    stat = path.stat()
    box, signature, contact = _camera_mesh(str(path), stat.st_mtime_ns, stat.st_size)
    rotation = instance['rotation']
    footprint = max((box[3] - box[0]) * math.sqrt(dot(rotation[0], rotation[0])),
                    (box[5] - box[2]) * math.sqrt(dot(rotation[2], rotation[2])))
    # CameraBlockers::new performs the same formula in f32 and fades when it
    # is <=4m. A small rounding allowance only rejects extra borrowing; it can
    # never classify a runtime-fadable prop as permanently visible.
    can_fade = not contact and footprint <= 4.0 + 1e-6 * max(1., footprint)
    return dict(can_fade=can_fade, footprint_m=footprint, geometry_sha256=signature, contact_surface=contact)


def camera_visibility_certificate(reference, source, identity):
    a = camera_descriptor(reference, reference.instances[identity])
    b = camera_descriptor(source, source.instances[identity])
    superset = not b['can_fade']
    identical = a['can_fade'] == b['can_fade'] and a['geometry_sha256'] == b['geometry_sha256']
    passed = superset or identical
    return dict(status='pass' if passed else 'unmeasured', complete=passed, reference=a, source=b,
                method='source_visibility_superset' if superset else 'identical_camera_fade' if identical else 'unproved_camera_fade')


def camera_visibility_identical(reference, source, identity):
    """Compatibility predicate: exact fade parity OR a proved source superset."""
    return camera_visibility_certificate(reference, source, identity)['status'] == 'pass'


def camera_problems(receipt):
    if not isinstance(receipt, dict) or receipt.get('status') != 'pass' or receipt.get('complete') is not True:
        return ['missing camera visibility certificate']
    for key in ('reference', 'source'):
        row = receipt.get(key)
        if (not isinstance(row, dict) or type(row.get('can_fade')) is not bool or type(row.get('contact_surface')) is not bool
                or type(row.get('footprint_m')) not in (int,float) or not math.isfinite(row['footprint_m']) or row['footprint_m']<0
                or not isinstance(row.get('geometry_sha256'),str) or len(row['geometry_sha256']) != 64):
            return ['invalid camera visibility descriptor']
        expected = not row['contact_surface'] and row['footprint_m'] <= 4.+1e-6*max(1.,row['footprint_m'])
        if row['can_fade'] != expected:
            return ['camera fade policy does not match descriptor']
    a,b = receipt['reference'],receipt['source']
    if receipt.get('method') == 'source_visibility_superset' and not b['can_fade']:
        return []
    if (receipt.get('method') == 'identical_camera_fade' and a['can_fade'] and b['can_fade']
            and a['geometry_sha256'] == b['geometry_sha256']):
        return []
    return ['unproved one-sided camera visibility']


@lru_cache(maxsize=16)
def placed_surface(paths, rotation, translation):
    local = surface_union(paths)
    transform = dict(rotation=rotation, translation=translation)
    vertices = [placed(point, transform) for point in local.vertices]
    lookup = {point: i for i, point in enumerate(local.vertices)}
    indices = [lookup[point] for triangle in local.triangles for point in triangle]
    return Surface(vertices, indices)


def object_surface(paths, instance):
    return placed_surface(tuple(str(path) for path in paths), tuple(tuple(column) for column in instance['rotation']),
                          tuple(instance['translation']))


class SceneSurface:
    """Two-level BVH; local mesh BVHs are shared and world BVHs loaded on demand."""
    def __init__(self, scene, layer, state, own_identity, domain_box=None):
        self.entries, self.nodes = [], []
        for identity, instance in scene.instances.items():
            # A base object has to work without borrowing a mode object. A
            # layer object may borrow the base and that same layer only.
            if identity[0] not in ({0, layer} if layer else {0}):
                continue
            try:
                geometry, model = scene.resolve(instance)
                if geometry is None or model is None:
                    continue
                states = scene.render_states(geometry,instance)
                own = identity == own_identity
                if own:
                    paths = states.get(state, states['model'])
                else:
                    cls = scene.resolve_class(instance)
                    if cls.get('base', '').lower() not in ('prop', 'building') or cls.get('door') or cls.get('turret_rig'):
                        continue
                    if not scene.borrow_lifetime(instance)['eligible']:
                        continue
                    if camera_descriptor(scene, instance)['can_fade']:
                        continue  # The chase camera can hide this whole prop.
                    paths = states['model']
                    if any(path is None or not model_descriptor(path, scene)['borrow_opaque'] for path in paths):
                        continue  # A cutout face cannot certify solid coverage.
                    primary = surface_union(tuple(str(path) for path in paths if path is not None))
                    # Never borrow a model that can switch to a different
                    # surface. This is a sufficient proof, not a visibility guess.
                    if any(any(path is None or not model_descriptor(path,scene)['borrow_opaque'] for path in variant)
                           or surface_union(tuple(str(path) for path in variant)).triangles != primary.triangles
                           for variant in states.values()):
                        continue
                if any(path is None for path in paths):
                    continue
                if any(not model_descriptor(path, scene)['all_surfaces_covered'] for path in paths):
                    continue  # Never contribute skipped/cutout triangles to a union.
                local = surface_union(tuple(str(path) for path in paths))
                box = local.bounds
                corners = [(box[a], box[b], box[c]) for a in (0, 3) for b in (1, 4) for c in (2, 5)]
                world_box = bounds([placed(point, instance) for point in corners])
                if domain_box is not None and any(world_box[i]>domain_box[i+3] or world_box[i+3]<domain_box[i] for i in range(3)):
                    continue
                self.entries.append((paths, instance, world_box))
            except (ValueError, OSError, KeyError, IndexError, TypeError):
                # A bad neighbor cannot be borrowed as evidence. The caller's
                # own placed-object record still fails on the bad input.
                continue
        if not self.entries:
            raise ValueError('no resolved scene render surfaces')
        self._build(list(range(len(self.entries))))
        self.bounds = self.nodes[0][0]

    def _build(self, entries):
        box = [min(self.entries[i][2][a] for i in entries) if a < 3
               else max(self.entries[i][2][a] for i in entries) for a in range(6)]
        index = len(self.nodes)
        self.nodes.append(None)
        if len(entries) <= 4:
            self.nodes[index] = (box, entries, None)
        else:
            axis = max(range(3), key=lambda a: box[a + 3] - box[a])
            entries.sort(key=lambda i: self.entries[i][2][axis] + self.entries[i][2][axis + 3])
            middle = len(entries) // 2
            left, right = self._build(entries[:middle]), self._build(entries[middle:])
            self.nodes[index] = (box, left, right)
        return index

    def closest_triangle(self, point):
        best, closest, triangle = math.inf, None, None
        queue = [(box_distance2(point, self.bounds), 0)]
        while queue:
            lower, index = heapq.heappop(queue)
            if lower > best * best:
                break
            _box, a, b = self.nodes[index]
            if b is None:
                for entry in a:
                    paths, instance, box = self.entries[entry]
                    if box_distance2(point, box) > best * best:
                        continue
                    candidate = object_surface(paths, instance).closest_triangle(point)
                    if candidate[0] < best:
                        best, closest, triangle = candidate
            else:
                for child in (a, b):
                    lower = box_distance2(point, self.nodes[child][0])
                    if lower <= best * best:
                        heapq.heappush(queue, (lower, child))
        return best, closest, triangle

    def closest(self, point):
        distance, closest, _triangle = self.closest_triangle(point)
        return distance, closest

    def distance(self, point):
        return self.closest_triangle(point)[0]

    def triangle_union_bound(self, triangle, tolerance):
        box = bounds(triangle)
        box = [value + (-tolerance if i < 3 else tolerance) for i, value in enumerate(box)]
        triangles = []
        for paths, instance, entry_box in self.entries:
            if any(entry_box[i] > box[i + 3] or entry_box[i + 3] < box[i] for i in range(3)):
                continue
            model = object_surface(paths, instance)
            for face in model.nearby_faces(box):
                triangles.append(model.triangles[face])
                if len(triangles) > 2048:
                    return None
        # Reuse the exact rational clipping implementation without constructing
        # a second BVH for the already spatially filtered candidate set.
        class CandidateSurface:
            def nearby_faces(self, _box):
                return range(len(self.triangles))
        candidates = CandidateSurface()
        candidates.triangles = triangles
        return Surface.triangle_union_bound(candidates, triangle, tolerance)


def _art_pair(certificate):
    if certificate.get('method') == 'identical_indexed_surface':
        return certificate, certificate
    if certificate.get('method') == 'visual_only_bidirectional_hausdorff':
        return certificate['reference_to_source'], certificate['source_to_reference']
    if 'art_forward' in certificate and 'art_reverse' in certificate:
        return certificate['art_forward'], certificate['art_reverse']
    return {}, {}


def scene_certificates(reference, source, identity, geometry, source_geometry, local, tolerance, max_cells):
    """Bound both complete placed surfaces against the eligible scene union.

    Passing every object's two directed bounds proves a Hausdorff bound on the
    complete scene. That bounds the collision residual increase without assuming
    a cross-source mesh partition or nearest-reference correspondence. A failed
    artwork bound is conservative/unmeasured, not a claimed physical offense.
    """
    instance, source_instance = reference.instances[identity], source.instances[identity]
    a_states, b_states = reference.render_states(geometry,instance), source.render_states(source_geometry,source_instance)
    results = {}
    for state, certificate in local.items():
        a_camera, b_camera = camera_descriptor(reference, instance), camera_descriptor(source, source_instance)
        if not camera_visibility_identical(reference, source, identity):
            results[state] = dict(status='unmeasured', complete=False,
                                  reason='different camera-fade geometry can hide different surfaces')
            continue
        a,b=a_states.get(state,a_states['model']),b_states.get(state,b_states['model'])
        if any(path is None for path in a+b):
            results[state]=dict(status='unmeasured',complete=False,reason='missing render-state model')
            continue
        forward,reverse=_art_pair(certificate)
        from render_gpu_bounds import placed_certificate,scene_bounds
        placed_proof=placed_certificate(reference,source,identity,a,b,forward,reverse,tolerance,max_cells)
        if placed_proof['status']=='pass':
            results[state]=placed_proof
            continue
        lifetime = dict(reference=reference.borrow_lifetime(instance), source=source.borrow_lifetime(source_instance))
        if any(value.get('eligible') is not True for value in lifetime.values()):
            results[state] = dict(status='unmeasured', complete=False,
                                  reason='own class lifetime is not proven static for scene borrowing', lifetime=lifetime)
            continue
        if a_camera['can_fade'] or b_camera['can_fade']:
            results[state] = dict(status='unmeasured', complete=False,
                                  reason='camera-fadable own model cannot borrow neighboring art')
            continue
        a, b = a_states.get(state, a_states['model']), b_states.get(state, b_states['model'])
        if any(path is None for path in a + b):
            results[state] = dict(status='unmeasured', complete=False, reason='missing render-state model')
            continue
        if any(not model_descriptor(path, scene)['all_surfaces_covered']
               for scene, paths in ((reference, a), (source, b)) for path in paths):
            results[state] = dict(status='unmeasured', complete=False,
                                  reason='partially drawn own model cannot borrow coverage with unsubmitted triangles')
            continue
        try:
            own_a,own_b=object_surface(a,instance),object_surface(b,source_instance)
            def domain(box):return [v+(-tolerance if i<3 else tolerance) for i,v in enumerate(box)]
            target_b = SceneSurface(source, identity[0], state, identity,domain(own_a.bounds))
            target_a = SceneSurface(reference, identity[0], state, identity,domain(own_b.bounds))
            gpu=scene_bounds(reference,source,identity,a,b,target_a,target_b)
            forward_budget=tolerance-gpu['forward_error_m'];reverse_budget=tolerance-gpu['reverse_error_m']
            if min(forward_budget,reverse_budget)<=0:
                results[state]=dict(status='unmeasured',complete=False,reason='scene GPU envelopes consume the added-gap budget',world_gpu_bounds=gpu)
                continue
            forward = art_certificate(own_a, target_b, forward_budget, max_cells)
            reverse = art_certificate(own_b, target_a, reverse_budget, max_cells)
            passed = forward['status'] == reverse['status'] == 'pass'
            results[state] = dict(status='pass' if passed else 'unmeasured', complete=passed,
                                  method='whole_scene_bidirectional_hausdorff', reference_to_source=forward,
                                  source_to_reference=reverse,
                                  lifetime=lifetime,world_gpu_bounds=gpu,
                                  reference_candidates=len(target_a.entries), source_candidates=len(target_b.entries))
            if not passed:
                results[state]['reason'] = 'scene-union artwork bound unresolved; not a confirmed physical gap'
        except (ValueError, OSError, KeyError, IndexError, TypeError) as exc:
            results[state] = dict(status='unmeasured', complete=False, reason=str(exc))
    return results
