"""World draw bounds for nonidentical static artwork.

For f32 unit roundoff u=2^-24, parsing each matrix component then evaluating
three products and three sums has Euclidean forward error below 16u*S, where
S=max_row_sum*max_local_coordinate+max_translation+1. We charge 32*f32_epsilon
=64u*S, including fused/unfused evaluation and gradual underflow. Identical
position/primitive/pose inputs cancel instead of paying two independent bounds.
Every matching static placement contributes to the worst budget; dynamic inputs
without an exact cancellation proof remain unresolved, never sampled.

Each row's exact dot-product magnitude is at most row_sum*extent. Rounding the
three stored coefficients and translation adds at most u*S; six subsequent
products/additions add gamma_6*S, gamma_6=6u/(1-6u). The Euclidean factor sqrt(3)
therefore stays below16u*S. FMA removes roundings; f64 metric evaluation and
GPU flush-to-zero errors fit in the remaining48u*S because S>=1. The explicit
2^124 magnitude limit keeps all intermediate sums away from f32 overflow.
"""
import math
from pathlib import Path

from world_fingerprint import digest,read_glb


def finite(value,minimum=0.):
    return type(value) in (int,float) and math.isfinite(value) and value>=minimum


def valid_ids(ids):
    return (isinstance(ids,list) and bool(ids)
            and all(isinstance(row,list) and len(row)==2 and all(type(v) is int and v>=0 for v in row) for row in ids)
            and len(set(map(tuple,ids)))==len(ids))


def envelope_problems(data):
    keys=('error_m','local_coordinate_extent_m','translation_extent_m','matrix_row_sum')
    if not isinstance(data,dict) or any(not finite(data.get(k)) for k in keys):
        return ['invalid world GPU envelope']
    magnitude=data['matrix_row_sum']*data['local_coordinate_extent_m']+data['translation_extent_m']+1.
    if not math.isfinite(magnitude) or magnitude>=2.**124:
        return ['unsupported overflowing world GPU transform']
    expected=32*(2.**-23)*magnitude
    return ['underreported world GPU envelope'] if data['error_m']<expected else []


def draw_envelope(paths,instance):
    from render_collision_check import surface_union
    from render_query_frame import f32
    local=surface_union(tuple(str(path) for path in paths))
    extent=max(1.,*(abs(v) for point in local.vertices for v in point))
    rotation=instance['rotation'];translation=instance['translation']
    values=[v for row in rotation for v in row]+list(translation)
    if any(type(v) not in (int,float) or not math.isfinite(v) for v in values):
        raise ValueError('nonfinite world draw transform')
    # S includes both the decimal-to-f32 matrix input conversion and the
    # GPU's affine evaluation, so the metric may keep its exact f64 domain.
    rows=max(sum(abs(v) for v in row) for row in zip(*rotation))
    movement=max(abs(v) for v in translation)
    for v in values:f32(v)
    bound=math.nextafter(32*(2.**-23)*(rows*extent+movement+1.),math.inf)
    result=dict(error_m=bound,local_coordinate_extent_m=extent,translation_extent_m=movement,matrix_row_sum=rows)
    if problems:=envelope_problems(result):raise ValueError('; '.join(problems))
    return result


def pose_partition(paths):
    from render_collision_check import Surface
    result=[]
    for path in paths:
        doc,accessor=read_glb(Path(path));parts=[]
        for mesh in doc.get('meshes',[]):
            for p in mesh.get('primitives',[]):
                positions=accessor(p['attributes']['POSITION']);indices=[i[0] for i in accessor(p['indices'])]
                parts.append(Surface(positions,indices).triangles)
        result.append(parts)
    return result


def exact_draw_inputs(reference,source,identity,a,b):
    from render_collision_check import surface_union
    x,y=reference.instances[identity],source.instances[identity]
    if any(x.get(k)!=y.get(k) for k in ('rotation','translation')):return False
    if surface_union(tuple(map(str,a))).triangles!=surface_union(tuple(map(str,b))).triangles:return False
    rc,sc=reference.resolve_class(x),source.resolve_class(y)
    if any(rc.get(k)!=sc.get(k) for k in ('door','turret_rig')):return False
    if rc.get('door') or rc.get('turret_rig'):
        return pose_partition(a)==pose_partition(b)
    return True


def group_bounds(reference,source,identity,a,b):
    from render_collision_check import dot,require_rigid
    if exact_draw_inputs(reference,source,identity,a,b):
        return dict(schema=1,status='pass',complete=True,method='identical_position_and_pose_inputs',
                    reference_error_m=0.,source_error_m=0.,stretch_upper=1.,placement_ids=[list(identity)])
    cache=getattr(source,'world_gpu_bounds_cache',None)
    if cache is None:source.world_gpu_bounds_cache=cache={}
    key=(reference.instances[identity]['class'],tuple(map(str,a)),tuple(map(str,b)))
    if key in cache:return cache[key]
    records=[];er=es=0.;stretch=1.
    for key_id,x in reference.instances.items():
        if x['class']!=reference.instances[identity]['class']:continue
        y=source.instances.get(key_id)
        if y is None:continue
        try:
            rg,_=reference.resolve(x);sg,_=source.resolve(y)
            rs,ss=reference.render_states(rg,x),source.render_states(sg,y)
            pairs=[(rs.get(state,rs['model']),ss.get(state,ss['model'])) for state in set(rs)|set(ss)]
            if not any(tuple(ra)==tuple(a) and tuple(sb)==tuple(b) for ra,sb in pairs):continue
            require_rigid(x);require_rigid(y)
            if x!=y:raise ValueError('world draw placement differs from corrected reference')
            life=[reference.borrow_lifetime(x),source.borrow_lifetime(y)]
            if any(v.get('eligible') is not True for v in life):
                raise ValueError('nonidentity moving/lifetime inputs lack an all-pose world draw bound')
            re,se=draw_envelope(a,x),draw_envelope(b,y)
            gram=[[dot(x['rotation'][i],x['rotation'][j]) for j in range(3)] for i in range(3)]
            scale=max(1.,math.sqrt(max(sum(abs(v) for v in row) for row in gram))+1e-12)
            er=max(er,re['error_m']);es=max(es,se['error_m']);stretch=max(stretch,scale)
            records.append(dict(identity=list(key_id),reference=re,source=se,stretch_upper=scale))
        except (KeyError,TypeError,ValueError,OSError) as exc:
            value=dict(schema=1,status='unmeasured',complete=False,reason=str(exc),placement_ids=[list(identity)])
            cache[key]=value;return value
    if not records:raise ValueError('world GPU placement domain empty')
    result=dict(schema=1,status='pass',complete=True,method='all_static_placement_roundoff_envelopes',
                reference_error_m=er,source_error_m=es,stretch_upper=stretch,placement_ids=[r['identity'] for r in records],
                placements=records,inputs_sha256=digest(records))
    cache[key]=result;return result


def bounds_problems(receipt,tolerance):
    if not isinstance(receipt,dict) or receipt.get('schema')!=1 or receipt.get('status')!='pass' or receipt.get('complete') is not True:
        return ['missing complete world GPU transform bound']
    if (not valid_ids(receipt.get('placement_ids')) or not finite(receipt.get('stretch_upper'),1.)
            or any(not finite(receipt.get(side+'_error_m')) for side in ('reference','source'))):
        return ['invalid world GPU placement bounds']
    if receipt.get('method')=='identical_position_and_pose_inputs':
        return [] if receipt['reference_error_m']==receipt['source_error_m']==0. and receipt['stretch_upper']==1. else ['invalid exact draw cancellation']
    if receipt.get('method')!='all_static_placement_roundoff_envelopes':return ['unknown world GPU bound method']
    rows=receipt.get('placements');ids=receipt.get('placement_ids')
    if (not isinstance(rows,list) or not rows or any(not isinstance(r,dict) for r in rows) or [r.get('identity') for r in rows]!=ids
            or digest(rows)!=receipt.get('inputs_sha256')):return ['incomplete world GPU placement catalog']
    for row in rows:
        if not finite(row.get('stretch_upper'),1.) or row['stretch_upper']>receipt['stretch_upper']:
            return ['world GPU stretch maximum omits a placement']
        for side in ('reference','source'):
            data=row.get(side,{})
            if problems:=envelope_problems(data):return problems
            if data['error_m']>receipt[side+'_error_m']:return ['world GPU maximum omits a placement']
    return []


def placed_certificate(reference,source,identity,a,b,forward,reverse,tolerance,max_cells):
    from render_collision_check import surface_union,art_certificate
    gpu=group_bounds(reference,source,identity,a,b)
    result=dict(status='unmeasured',complete=False,method='placed_object_bidirectional_hausdorff',world_gpu_bounds=gpu)
    if gpu['status']!='pass':return dict(result,reason=gpu.get('reason'))
    scale=gpu['stretch_upper'];guard=gpu['reference_error_m']+gpu['source_error_m']
    budget=(tolerance-guard)/scale
    if gpu['method']=='identical_position_and_pose_inputs':
        forward=reverse=dict(status='pass',complete=True,method='identical_indexed_surface',maximum_observed_m=0.)
    elif budget<=0:return dict(result,reason='world GPU draw envelopes consume the added-gap budget')
    else:
        def enough(value):
            upper=value.get('certified_upper_m',value.get('maximum_observed_m'))
            return value.get('status')=='pass' and upper is not None and upper<=budget
        if not enough(forward) or not enough(reverse):
            r,s=surface_union(tuple(map(str,a))),surface_union(tuple(map(str,b)))
            forward=art_certificate(r,s,budget,max_cells);reverse=art_certificate(s,r,budget,max_cells)
    passed=forward.get('status')==reverse.get('status')=='pass'
    result.update(status='pass' if passed else 'unmeasured',complete=passed,
                  reference_to_source=forward,source_to_reference=reverse,stretch_upper=scale)
    return result


def scene_bounds(reference,source,identity,a,b,target_a,target_b):
    own_a=draw_envelope(a,reference.instances[identity]);own_b=draw_envelope(b,source.instances[identity])
    refs=[draw_envelope(paths,instance) for paths,instance,_box in target_a.entries]
    srcs=[draw_envelope(paths,instance) for paths,instance,_box in target_b.entries]
    return dict(schema=1,status='pass',complete=True,method='eligible_scene_candidate_envelopes',
                reference_own=own_a,source_own=own_b,reference_targets=refs,source_targets=srcs,
                forward_error_m=own_a['error_m']+max(v['error_m'] for v in srcs),
                reverse_error_m=own_b['error_m']+max(v['error_m'] for v in refs))


def scene_bounds_problems(receipt):
    if (not isinstance(receipt,dict) or receipt.get('schema')!=1 or receipt.get('status')!='pass'
            or receipt.get('complete') is not True or receipt.get('method')!='eligible_scene_candidate_envelopes'):
        return ['missing scene GPU envelope']
    refs,srcs=receipt.get('reference_targets'),receipt.get('source_targets')
    if not isinstance(refs,list) or not refs or not isinstance(srcs,list) or not srcs:
        return ['missing scene GPU candidates']
    values=[receipt.get('reference_own'),receipt.get('source_own'),*refs,*srcs]
    for row in values:
        if problems:=envelope_problems(row):return problems
    forward=values[0]['error_m']+max(v['error_m'] for v in srcs)
    reverse=values[1]['error_m']+max(v['error_m'] for v in refs)
    if (not finite(receipt.get('forward_error_m')) or not finite(receipt.get('reverse_error_m'))
            or receipt['forward_error_m']!=forward or receipt['reverse_error_m']!=reverse):
        return ['scene GPU sums omit a candidate']
    return []
