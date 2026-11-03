"""Sufficient absolute contact certificate, independent of PS2 art detail.

If every actual map collision surface is within the budget of definitely drawn
art, and every potentially drawn face is within it of actual collision, then the
extra gap over any nonnegative PS2 residual is at most that budget. This path
borrows no neighbors and requires immutable, nonfading own art in every state.
Every primitive is replaced only inside this instrument by a complete analytic
proxy with a proved Hausdorff error. Raw ray frames and normalized contact frames
are enclosed with outward f32 intervals; their error is charged to the budget.
"""
from functools import lru_cache
import math
from pathlib import Path

from world_fingerprint import digest, read_glb, safe_child
from render_model_dependencies import model_descriptor

BODY_BASIS = 'assets::body_layers ordered soldier|ordnance; vehicles/flyers world contacts use soldier; hover ground/bolts use ordnance; VEHICLE is reserved'


def layer_bodies(bodies, props, layer):
    bit = {'soldier':1, 'ordnance':16}[layer]
    kept = [mask == 0 or bool(mask & bit) for _name, mask in bodies]
    accumulator = [False] * len(bodies)
    for key, name in props:
        index = next((i for i, (body, _mask) in enumerate(bodies) if body == name), None)
        if key == layer:
            if name in ('none', 'clear'):
                accumulator = [False] * len(bodies); kept = accumulator.copy()
            elif index is not None:
                accumulator[index] = True; kept = accumulator.copy()
        elif key == 'vehicle_only' and index is not None:
            kept[index] = False
    return kept


def axis_frame(frame):
    columns = frame['rotation']
    return (len(columns) == 3 and all(len(c) == 3 for c in columns)
            and all(v in (-1.,0.,1.) for c in columns for v in c)
            and all(sum(abs(v) for v in c) == 1 for c in columns)
            and all(sum(abs(columns[c][r]) for c in range(3)) == 1 for r in range(3)))


def surface_from_triangles(triangles):
    from render_collision_check import Surface
    vertices = [p for tri in triangles for p in tri]
    return Surface(vertices, list(range(len(vertices)))) if vertices else None


def draw_surfaces(scene, paths):
    """Potential faces are an outer envelope, covered faces an inner one."""
    from render_collision_check import Surface
    potential, covered, records = [], [], []
    tiers = ('ps2',) if scene.reference_art else ('ps2','pc')
    for path in paths:
        descriptor = model_descriptor(path, scene); doc, accessor = read_glb(Path(path))
        parts = [part for mesh in doc.get('meshes', []) for part in mesh.get('primitives', [])]
        for ordinal, part in enumerate(parts):
            positions = accessor(part['attributes']['POSITION']); indices = [p[0] for p in accessor(part['indices'])]
            triangles = Surface(positions, indices).triangles
            states = [r['coverage'] for r in descriptor['records'] if r['primitive'] == ordinal]
            if len(states) != len(tiers):
                raise ValueError('incomplete effective draw tier proof')
            if any(state != 'absent' for state in states): potential.extend(triangles)
            if all(state == 'full' for state in states): covered.extend(triangles)
            records.append(dict(model=Path(path).name, primitive=ordinal, tiers=list(tiers), coverage=states,
                                triangles=len(triangles)))
    return surface_from_triangles(potential), surface_from_triangles(covered), records


class ContactSurface:
    def __init__(self, scene, identity, strict_frame=True):
        from render_collision_check import primitive_patches, surface, dot, sub
        self.scene, self.identity = scene, identity
        instance = scene.instances[identity]; cls = scene.resolve_class(instance)
        geometry, _model = scene.resolve(instance)
        self.catalog, self.patches, self.primitives, triangles = [], [], [], []
        if cls.get('base','').lower() == 'trap' or geometry not in scene.collisions:
            self.mesh = None; return
        folder, entry = scene.collisions[geometry]
        rows = ([('collisionmesh',0)] if entry.get('glb') else []) + [(str(p['name']).lower(),p.get('mask',0)) for p in entry.get('prims',[])]
        if any(type(mask) not in (int,float) or not math.isfinite(mask) or int(mask)!=mask or not 0<=mask<=255 for _name,mask in rows):
            raise ValueError('invalid authored collision mask')
        rows=[(name,int(mask)) for name,mask in rows]
        props = cls.get('collision',[])
        soldier, ordnance = layer_bodies(rows,props,'soldier'), layer_bodies(rows,props,'ordnance')
        offset = 0
        if entry.get('glb'):
            active = soldier[0] or ordnance[0]; mesh = surface(str(safe_child(folder,entry['glb'])))
            # Rejected or degenerate parry input cannot become phantom contact.
            for a,b,c in mesh.triangles:
                ab,ac=sub(b,a),sub(c,a); area=dot(ab,ab)*dot(ac,ac)-dot(ab,ac)**2
                if area <= 0: raise ValueError('collision mesh has a degenerate face; runtime acceptance not proved')
            self.catalog.append(dict(name='collisionmesh',kind='mesh',soldier=soldier[0],ordnance=ordnance[0],runtime_built=bool(active)))
            if active: triangles.extend(mesh.triangles)
            offset=1
        for i, primitive in enumerate(entry.get('prims',[])):
            active = soldier[offset+i] or ordnance[offset+i]
            kind = primitive['kind']; built=active and kind in ('box','sphere','cylinder')
            self.catalog.append(dict(name=primitive['name'],kind=kind,mask=primitive.get('mask',0),soldier=soldier[offset+i],
                                     ordnance=ordnance[offset+i],runtime_built=built,location=primitive['translation'],dims=primitive['dims']))
            if not built: continue
            if strict_frame and not axis_frame(primitive):
                raise ValueError('absolute contact requires a proved primitive query frame')
            patches=list(primitive_patches(primitive,i))
            self.patches.extend(patches)
            if kind == 'box': triangles.extend(p[1] for p in patches)
            else: self.primitives.append(primitive)
        self.mesh=surface_from_triangles(triangles)
        # Box patches are already in the primitive list; avoid duplicating them.
        from render_collision_check import triangle_patch
        if entry.get('glb') and (soldier[0] or ordnance[0]):
            self.patches.extend(triangle_patch(tri,('active_collision_triangle',i)) for i,tri in enumerate(mesh.triangles))

    def upper(self, point):
        from render_collision_check import dot,sub,placed,triangle_distance_upper
        values=[]
        if self.mesh is not None:
            _distance,_closest,triangle=self.mesh.closest_triangle(point)
            values.append(triangle_distance_upper(point,triangle))
        for p in self.primitives:
            local=[dot(column,sub(point,p['translation'])) for column in p['rotation']]
            r=p['dims'][0]
            if p['kind']=='sphere':
                length=math.sqrt(dot(local,local))
                q=[r,0.,0.] if length==0 else [v*r/length for v in local]
            else:
                rho=math.hypot(local[0],local[2]);half=p['dims'][1]/2
                radial=[r,0.] if rho==0 else [local[0]*r/rho,local[2]*r/rho]
                if rho>=r or abs(local[1])>=half:
                    q=[radial[0],max(-half,min(half,local[1])),radial[1]] if rho>=r else [local[0],math.copysign(half,local[1]),local[2]]
                elif r-rho <= half-abs(local[1]): q=[radial[0],local[1],radial[1]]
                else: q=[local[0],math.copysign(half,local[1]),local[2]]
            # Projection uses a bounded number of f64 operations in an exact
            # signed-axis frame. This guard bounds their point-on-shape error.
            error=1e-12*max(1.,*(abs(v) for v in point),*p['dims'])
            values.append(math.nextafter(math.dist(point,placed(q,p))+error,math.inf))
        if not values: raise ValueError('no actual active collision surface')
        return min(values)


def absolute_contact(reference, source, identity, paths, tolerance, max_cells):
    from render_collision_check import art_certificate,certify_surface,triangle_patch,triangle_distance_upper
    from render_scene_surface import camera_descriptor
    result=dict(method='absolute_active_contact_bound',status='unmeasured',complete=False,body_basis=BODY_BASIS)
    try:
        a,b=reference.instances[identity],source.instances[identity]
        lifetime=dict(reference=reference.borrow_lifetime(a),source=source.borrow_lifetime(b))
        if any(v.get('eligible') is not True for v in lifetime.values()):
            raise ValueError('absolute contact requires static lifetime in reference and source')
        if camera_descriptor(source,b)['can_fade']:
            raise ValueError('absolute contact cannot borrow a camera-hidden own surface')
        inventory=ContactSurface(reference,identity,strict_frame=False).catalog
        identical=all(row['soldier']==row['ordnance'] for row in inventory if row['runtime_built'])
        result.update(body_catalog=inventory,query_domains_identical=identical,
                      query_domains=['soldier','ordnance'],domain_relation='identical_runtime_body_membership')
        if not identical:
            raise ValueError('soldier and ordnance contact domains differ; a union cannot prove either layer')
        from render_contact_groups import group_context
        cache=getattr(source,'contact_group_cache',None)
        if cache is None:source.contact_group_cache=cache={}
        geometry,_model=reference.resolve(a)
        cache_key=(geometry,reference.collision_identity(geometry),digest(reference.collision_policy(a)),
                   tuple((str(path),Path(path).stat().st_mtime_ns,Path(path).stat().st_size) for path in paths),tolerance,max_cells)
        if cache_key in cache:return cache[cache_key]
        context=group_context(reference,source,identity,paths,tolerance)
        proxy,potential,covered=context['proxy'],context['potential'],context['covered']
        guard,stretch=context['error'],context['stretch']
        budget=(tolerance-guard)/stretch
        if budget<=0: raise ValueError('query/analytic/draw bounds consume absolute contact budget')
        forward=art_certificate(proxy,covered,budget,max_cells)
        reverse=art_certificate(potential,proxy,budget,max_cells)
        passed=forward['status']==reverse['status']=='pass'
        result.update(status='pass' if passed else 'unmeasured',complete=passed,collision_to_art=forward,art_to_collision=reverse,
                      numeric_guard_m=guard,stretch_upper=stretch,lifetime=context['lifetime'],query_frames='outward_f32_all_query_frame_intervals_v2',
                      camera_fade_absent=True,body_catalog=context['catalog'],body_catalog_sha256=digest(context['catalog']),draws=context['draws'],
                      analytic_proxy_error_m=context['proxy_error'],query_frame_evidence=context['frames'],
                      placement_bounds=context['placement_bounds'],placement_ids=context['placement_ids'],
                      domain_instances_sha256=context['domain_instances_sha256'],worst_query_frame=context['worst_query_frame'],
                      residual_increase_certified_upper_m=max(forward.get('certified_upper_m',math.inf),reverse.get('certified_upper_m',math.inf))*stretch+guard if passed else None)
        if not passed: result['reason']='absolute contact bound unresolved; this does not prove a regression over PS2'
        cache[cache_key]=result
    except (ValueError,OSError,KeyError,TypeError,IndexError) as exc: result['reason']=str(exc)
    return result


def contact_problems(receipt,tolerance):
    from render_collision_check import certificate_problems
    if (receipt.get('status')!='pass' or receipt.get('complete') is not True or receipt.get('body_basis')!=BODY_BASIS
            or receipt.get('query_frames')!='outward_f32_all_query_frame_intervals_v2' or receipt.get('camera_fade_absent') is not True
            or receipt.get('query_domains_identical') is not True or receipt.get('query_domains')!=['soldier','ordnance']
            or receipt.get('domain_relation')!='identical_runtime_body_membership'):
        return ['incomplete absolute active contact proof']
    guard=receipt.get('numeric_guard_m');upper=receipt.get('residual_increase_certified_upper_m');stretch=receipt.get('stretch_upper')
    if type(guard) not in (int,float) or not math.isfinite(guard) or not 0<=guard<tolerance or type(upper) not in (int,float) or not math.isfinite(upper) or upper>tolerance or type(stretch) not in (int,float) or not math.isfinite(stretch) or stretch<1:
        return ['invalid absolute contact bound/float guard']
    catalog=receipt.get('body_catalog');life=receipt.get('lifetime');draws=receipt.get('draws')
    if (not isinstance(catalog,list) or not catalog or digest(catalog)!=receipt.get('body_catalog_sha256')
            or not any(r.get('runtime_built') is True for r in catalog)
            or any(type(r.get('soldier')) is not bool or type(r.get('ordnance')) is not bool or type(r.get('runtime_built')) is not bool
                   or (r['runtime_built'] and r['soldier']!=r['ordnance']) for r in catalog)
            or not isinstance(life,dict) or set(life)!={'reference','source'}
            or any(v.get('eligible') is not True or not isinstance(v.get('class_inputs_sha256'),str) or len(v['class_inputs_sha256'])!=64
                   or type(v.get('class_files')) is not int or v['class_files']<1 for v in life.values())
            or not isinstance(draws,list) or not draws
            or not isinstance(receipt.get('query_frame_evidence'),list) or not receipt['query_frame_evidence']):
        return ['missing actual collision, draw or lifetime domain evidence']
    placements=receipt.get('placement_bounds');ids=receipt.get('placement_ids')
    if (not isinstance(placements,list) or not placements or not isinstance(ids,list)
            or [r.get('identity') for r in placements]!=ids or len({tuple(i) for i in ids})!=len(ids)
            or not isinstance(receipt.get('domain_instances_sha256'),str) or len(receipt['domain_instances_sha256'])!=64
            or any(type(r.get('combined_error_m')) not in (int,float) or not 0<=r['combined_error_m']<=guard
                   or type(r.get('stretch_upper')) not in (int,float) or not 1<=r['stretch_upper']<=stretch for r in placements)):
        return ['missing all-placement contact domain bounds']
    proxy=receipt.get('analytic_proxy_error_m')
    if type(proxy) not in (int,float) or not math.isfinite(proxy) or proxy<0:
        return ['invalid analytic proxy error bound']
    for row in placements:
        if (type(row.get('query_error_m')) not in (int,float) or not math.isfinite(row['query_error_m']) or row['query_error_m']<0
                or type(row.get('draw_error_m')) not in (int,float) or not math.isfinite(row['draw_error_m']) or row['draw_error_m']<0
                or row['combined_error_m']<row['query_error_m']+proxy*row['stretch_upper']+row['draw_error_m']):
            return ['contact placement error components exceed the reported guard']
    return [problem for name in ('collision_to_art','art_to_collision') for problem in certificate_problems(receipt.get(name),(tolerance-guard)/stretch)]


def supplement_states(reference,source,identity,source_states,proofs,opacity,tolerance,max_cells):
    """Try the independent sufficient bound only where art correspondence failed."""
    result=dict(proofs)
    for state,proof in proofs.items():
        if proof.get('status')=='pass' and opacity.get(state,{}).get('status')=='pass':continue
        paths=source_states.get(state,source_states['model'])
        if any(path is None for path in paths):continue
        absolute=absolute_contact(reference,source,identity,paths,tolerance,max_cells)
        if absolute['status']=='pass':
            result[state]=dict(absolute,reference_art_attempt=proof)
        else:
            result[state]=dict(proof,absolute_contact_attempt=absolute)
    return result
