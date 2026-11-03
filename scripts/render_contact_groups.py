"""All-placement bounds for reusing one local contact surface certificate.

The maximum includes every matching placed object in every mode layer. No pose
is sampled. Each instance has its own actual body-frame and draw-roundoff bound;
the local continuous distance budget is reduced by the worst of those bounds.
"""
import copy
import math
from pathlib import Path

from world_fingerprint import digest
from render_query_frame import f32,query_frame


def group_context(reference,source,identity,paths,tolerance):
    from render_contact_proxy import proxy_surface
    from render_contact_check import draw_surfaces
    from render_scene_surface import camera_descriptor
    inst=reference.instances[identity];geometry,_=reference.resolve(inst)
    policy=reference.collision_policy(inst);src_geometry,_=source.resolve(source.instances[identity])
    if source.collision_identity(src_geometry)!=reference.collision_identity(geometry):
        raise ValueError('absolute contact source collision differs')
    if source.collision_policy(source.instances[identity])!=policy:
        raise ValueError('absolute contact source body policy differs')
    members=[]
    wanted=tuple(Path(p) for p in paths)
    for key,instance in reference.instances.items():
        try:
            rg,_=reference.resolve(instance)
            if rg!=geometry or reference.collision_policy(instance)!=policy:continue
            other=source.instances[key];sg,_=source.resolve(other)
            if sg!=src_geometry or source.collision_policy(other)!=policy:continue
            variants=source.render_states(sg,other)
            if not any(tuple(variant)==wanted for variant in variants.values()):continue
            if other!=instance:raise ValueError('matching contact placement differs from corrected reference')
            members.append(key)
        except KeyError:
            continue  # This object's own mandatory gate row retains the error.
    if identity not in members:raise ValueError('absolute contact instance absent from exhaustive group')
    local=copy.copy(reference);local.instances=dict(reference.instances)
    local.instances[identity]=dict(inst,rotation=[[1,0,0],[0,1,0],[0,0,1]],translation=[0,0,0])
    proxy,proxy_error,catalog,frames=proxy_surface(local,identity,tolerance)
    potential,covered,draws=draw_surfaces(source,paths)
    if potential is None or covered is None:raise ValueError('absolute contact lacks guaranteed drawn art')
    extent=max(1.,*(abs(v) for p in potential.vertices for v in p))
    _folder,collision=reference.collisions[geometry]
    bounds=[];worst_error=0.;stretch_max=1.;raw_inputs=[];lifetimes=None;worst_frame=None
    for key in members:
        a,b=reference.instances[key],source.instances[key]
        life=dict(reference=reference.borrow_lifetime(a),source=source.borrow_lifetime(b))
        if any(v.get('eligible') is not True for v in life.values()):
            raise ValueError('contact group contains an unproved lifetime')
        if camera_descriptor(source,b)['can_fade']:
            raise ValueError('contact group contains camera-fadable source art')
        lifetimes=life
        from render_collision_check import require_rigid
        require_rigid(a)
        require_rigid(b)
        columns=[[f32(v) for v in c] for c in a['rotation']]
        gram=[[sum(columns[i][k]*columns[j][k] for k in range(3)) for j in range(3)] for i in range(3)]
        stretch=math.sqrt(max(sum(abs(v) for v in row) for row in gram))+1e-12
        stretch=max(1.,stretch);stretch_max=max(stretch_max,stretch)
        body_error=0.;body_at=None
        for frame in frames:
            index=frame.get('primitive_index')
            prim=None if index is None else collision['prims'][index]
            actual=query_frame(a,prim)
            error=actual['rotation_error_per_unit']*frame['radius_m']+actual['translation_error_m']
            if error>=body_error:body_error,body_at=error,dict(body=frame['body'],**actual)
        row_sum=max(sum(abs(columns[c][r]) for c in range(3)) for r in range(3))
        draw_error=math.nextafter(32*(2.**-23)*(row_sum*extent+max(abs(f32(v)) for v in a['translation'])+1.),math.inf)
        error=body_error+proxy_error*stretch+draw_error
        if error>=worst_error:worst_error=error;worst_frame=dict(identity=list(key),**(body_at or {}))
        bounds.append(dict(identity=list(key),stretch_upper=stretch,query_error_m=body_error,
                           draw_error_m=draw_error,combined_error_m=error))
        raw_inputs.append([list(key),a,b,life])
    return dict(proxy=proxy,potential=potential,covered=covered,draws=draws,catalog=catalog,frames=frames,
                error=worst_error,stretch=stretch_max,lifetime=lifetimes,placement_bounds=bounds,
                placement_ids=[list(key) for key in members],domain_instances_sha256=digest(raw_inputs),
                worst_query_frame=worst_frame,proxy_error=proxy_error)
