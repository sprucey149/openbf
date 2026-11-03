"""Prove zero differential for defined native anomalies with identical inputs.

This never repairs a reference body or assumes how an invalid signed box ought
to work. The same complete physics, control, model and draw inputs reach the
same runtime program in both cases, independently of their filesystem roots. Actual loaded
base/enhanced draw coverage must be proved equal, including the
renderer's reduced-texture fallback and material skip policy.
"""
import hashlib
import math

from world_fingerprint import digest, glb_geometry, read_glb, safe_child


def control_fingerprints(scene, identity, geometry):
    """Path-independent input identities, preserving signed zero and raw bytes."""
    inst=scene.instances[identity];folder,collision=scene.collisions[geometry]
    classes=scene.class_inputs();cls=scene.resolve_class(inst)
    chains=[[name.lower(),classes.chain(name)] for name in dict.fromkeys([inst['class'],cls['name']])]
    # These two draw-tier controls are proved through the actual loaded texture
    # registry and effective coverage below; their different names/bytes alone
    # do not change contact art when every selected surface has equal coverage.
    controls={key:value for key,value in scene.doc.items() if key not in ('texture_tier2','texture_tiers')}
    return dict(manifest_sha256=digest(controls),merged_classes_sha256=digest(scene.classes),
                merged_geometries_sha256=digest(scene.geometries),
                placements_sha256=digest([[list(key),value] for key,value in scene.instances.items()]),
                raw_class_chains_sha256=digest(chains),
                resolved_policy_sha256=digest(scene.collision_policy(inst)),
                collision_semantic_sha256=scene.collision_identity(geometry),collision_record_sha256=digest(collision),
                collision_glb_sha256=hashlib.sha256(safe_child(folder,collision['glb']).read_bytes()).hexdigest()
                if collision.get('glb') else None)


def native_anomaly_identity(reference, source, identity, error):
    try:
        return native_identity(reference,source,identity,error)
    except (ValueError,OSError,KeyError,TypeError,IndexError,AttributeError):
        return None


def native_identity(reference,source,identity,error):
    text = str(error)
    missing_texture = text.startswith('nonopaque material has no measured texture:')
    signed_box = text.startswith('invalid collision primitive dimensions: box')
    if not (missing_texture or signed_box):return None
    inst=reference.instances[identity];other=source.instances[identity]
    geometry,model=reference.resolve(inst);source_geometry,source_model=source.resolve(other)
    if model is None or source_model is None:return None
    a=control_fingerprints(reference,identity,geometry)
    b=control_fingerprints(source,identity,source_geometry)
    # Digest equality is intentional: Python's +0.0 == -0.0 is insufficient
    # for a claim about identical deterministic runtime inputs.
    if a!=b:return None
    if signed_box:
        _folder,entry=reference.collisions[geometry]
        bad=[p for p in entry.get('prims',[]) if p.get('kind')=='box' and len(p.get('dims',[]))==3
             and all(type(value) in (int,float) and math.isfinite(value) for value in p['dims'])
             and any(value<0 for value in p['dims'])]
        if not bad:return None
    ref_states=reference.render_states(geometry,inst);src_states=source.render_states(source_geometry,other)
    if set(ref_states)!=set(src_states):return None
    from render_model_dependencies import model_dependencies,dependency_problems
    dependencies={};model_files={}
    for state,paths in ref_states.items():
        donors=src_states[state]
        if len(paths)!=len(donors) or any(path is None for path in paths+donors):return None
        pairs=[]
        for path,donor in zip(paths,donors):
            raw=path.read_bytes();sha=hashlib.sha256(raw).hexdigest()
            if hashlib.sha256(donor.read_bytes()).hexdigest()!=sha:return None
            doc,_accessor=read_glb(path)
            if doc.get('asset',{}).get('version')!='2.0' or doc.get('extensionsRequired'):return None
            positions,indices=glb_geometry(path)
            if (not positions or not indices or len(indices)%3
                    or any(len(p)!=3 or not all(math.isfinite(v) for v in p) for p in positions)
                    or any(type(i) is not int or not 0<=i<len(positions) for i in indices)):return None
            if any(mat.get('alphaMode','OPAQUE') not in ('OPAQUE','MASK','BLEND') for mat in doc.get('materials',[])):return None
            pairs.append(dict(reference_sha256=sha,source_sha256=sha))
        receipt=model_dependencies(reference,source,paths,donors)
        if receipt['status']!='pass' or dependency_problems(receipt):return None
        dependencies[state]=receipt;model_files[state]=pairs
    return dict(status='baseline_input_identity',baseline='Exact physics, control, model and effective draw identity proves zero increase; '
                'the native anomaly is retained and no absolute surface interpretation is inferred',
                layer_ordinal=identity[0],index=identity[1],name=inst.get('name',''),class_name=inst['class'],
                translation=inst['translation'],geometry=geometry,placement_identity=list(identity),
                placement_input=inst,placement_input_sha256=digest(inst),
                anomaly_kind='signed_box_dimensions' if signed_box else 'missing_texture_resolution',
                native_anomaly=text,reference_identical_input_root=reference.assets.resolve()==source.assets.resolve(),
                physics_and_visible_inputs_identical=True,input_identity_evidence=dict(reference=a,source=b),model_files=model_files,
                raw_class_inventories={label:dict(sha256=scene.class_inputs().sha256,files=scene.class_inputs().files)
                                       for label,scene in [('reference',reference),('source',source)]},
                actual_draw_coverage_identical=True,world_draw_dependencies=dependencies,manifest_class_geometry_identity=True,
                render_program_inputs_sha256=digest(model_files),residual_increase_certified_upper_m=0.,
                absolute_visible_residual_measured=False,class_collision_identical=True,collision_identical=True)


def identity_problems(row):
    if (type(row.get('reference_identical_input_root')) is not bool or row.get('physics_and_visible_inputs_identical') is not True
            or row.get('actual_draw_coverage_identical') is not True
            or row.get('manifest_class_geometry_identity') is not True
            or type(row.get('residual_increase_certified_upper_m')) not in (int,float) or row['residual_increase_certified_upper_m'] != 0.
            or row.get('absolute_visible_residual_measured') is not False
            or row.get('class_collision_identical') is not True or row.get('collision_identical') is not True
            or row.get('anomaly_kind') not in ('signed_box_dimensions', 'missing_texture_resolution')
            or not isinstance(row.get('render_program_inputs_sha256'), str) or len(row['render_program_inputs_sha256']) != 64
            or not row.get('native_anomaly') or not row.get('baseline')):
        return ['missing exact-input evidence for native anomaly baseline']
    evidence=row.get('input_identity_evidence');models=row.get('model_files')
    inst=row.get('placement_input');identity=row.get('placement_identity')
    if (not isinstance(inst,dict) or digest(inst)!=row.get('placement_input_sha256')
            or not isinstance(identity,list) or len(identity)!=2 or any(type(i) is not int or i<0 for i in identity)
            or identity!=[row.get('layer_ordinal'),row.get('index')] or inst.get('class')!=row.get('class_name')
            or inst.get('name','')!=row.get('name') or digest(inst.get('translation'))!=digest(row.get('translation'))
            or not isinstance(row.get('geometry'),str)):
        return ['native identity proof omits or changes this placed object']
    if (not isinstance(evidence,dict) or set(evidence)!={'reference','source'}
            or not isinstance(evidence['reference'],dict) or evidence['reference']!=evidence['source']):
        return ['missing path-independent native physics/control identity']
    expected={'manifest_sha256','merged_classes_sha256','merged_geometries_sha256','placements_sha256',
              'raw_class_chains_sha256','resolved_policy_sha256','collision_semantic_sha256',
              'collision_record_sha256','collision_glb_sha256'}
    value=evidence['reference']
    if (set(value)!=expected
            or any(not isinstance(value[key],str) or len(value[key])!=64 for key in expected-{'collision_glb_sha256'})
            or (value['collision_glb_sha256'] is not None and (not isinstance(value['collision_glb_sha256'],str) or len(value['collision_glb_sha256'])!=64))):
        return ['invalid native physics/control input fingerprints']
    raw=row.get('raw_class_inventories')
    if (not isinstance(raw,dict) or set(raw)!={'reference','source'}
            or any(not isinstance(v,dict) or not isinstance(v.get('sha256'),str) or len(v['sha256'])!=64
                   or type(v.get('files')) is not int or v['files']<1 for v in raw.values())):
        return ['missing complete raw class inventory fingerprints']
    if (not isinstance(models,dict) or not models or digest(models)!=row['render_program_inputs_sha256']
            or any(not isinstance(pairs,list) or not pairs or any(not isinstance(pair,dict)
                    or not isinstance(pair.get('reference_sha256'),str) or len(pair['reference_sha256'])!=64
                    or pair.get('source_sha256')!=pair['reference_sha256'] for pair in pairs) for pairs in models.values())):
        return ['missing exact model/pose bytes for native anomaly']
    from render_model_dependencies import dependency_problems
    dependencies = row.get('world_draw_dependencies')
    if not isinstance(dependencies, dict) or set(dependencies)!=set(models):
        return ['missing actual native anomaly draw dependency proof']
    problems = []
    for value in dependencies.values():
        problems.extend(dependency_problems(value))
        if value.get('status') != 'pass':
            problems.append('different native anomaly draw coverage')
    return problems
