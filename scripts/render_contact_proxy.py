"""Complete analytic surface approximation with an explicit Hausdorff budget.

Cylinder sides and complete caps use a regular inscribed prism; its Hausdorff
error is r*(1-cos(pi/N)). A sphere uses a complete radially projected octahedral
triangulation. Every facet lies in the cap about a vertex of angular radius at
most its largest edge angle, so r*(1-min(edge dot)) bounds both radial directions.
No face, body or domain is sampled away. Noncertifiable transforms fail closed.
"""
from functools import lru_cache
import math

from render_query_frame import f32,query_frame
from world_fingerprint import safe_child


def norm(v):return math.sqrt(sum(x*x for x in v))
def unit(v):
    length=norm(v)
    if not length:raise ValueError('zero proxy radial vector')
    return tuple(x/length for x in v)


@lru_cache(maxsize=128)
def analytic_proxy(kind,dims,budget):
    r=dims[0]
    if kind=='box':
        from render_collision_check import primitive_patches
        p=dict(kind='box',dims=dims,name='proxy',rotation=[[1,0,0],[0,1,0],[0,0,1]],translation=[0,0,0])
        return tuple(patch[1] for patch in primitive_patches(p,0)),0.
    if r<=0 or budget<=0:raise ValueError('degenerate analytic body has no certified proxy')
    if kind=='cylinder':
        half=dims[1]/2
        if half<=0:raise ValueError('degenerate analytic cylinder')
        angle=math.acos(max(-1.,min(1.,1-budget/r)))
        count=max(8,math.ceil(math.pi/angle)) if angle else 1000001
        count=1<<(count-1).bit_length()
        if count>100000:raise ValueError('analytic cylinder proxy work limit')
        ring=[(r*math.cos(math.tau*i/count),r*math.sin(math.tau*i/count)) for i in range(count)]
        triangles=[]
        for i,(x,z) in enumerate(ring):
            nx,nz=ring[(i+1)%count]
            a,b,c,d=(x,-half,z),(nx,-half,nz),(nx,half,nz),(x,half,z)
            triangles.extend(((a,b,c),(a,c,d),((0.,-half,0.),b,a),((0.,half,0.),d,c)))
        error=r*(1-math.cos(math.pi/count))+1e-12*max(1.,r,half)
        return tuple(triangles),math.nextafter(error,math.inf)
    if kind!='sphere':raise ValueError('unsupported proxy primitive')
    xp,xn,yp,yn,zp,zn=(1.,0.,0.),(-1.,0.,0.),(0.,1.,0.),(0.,-1.,0.),(0.,0.,1.),(0.,0.,-1.)
    triangles=[(y,x,z) for y in (yp,yn) for x,z in ((xp,zp),(zp,xn),(xn,zn),(zn,xp))]
    while True:
        minimum=min(sum(a[i]*b[i] for i in range(3)) for tri in triangles for a,b in ((tri[0],tri[1]),(tri[1],tri[2]),(tri[2],tri[0])))
        error=r*(1-minimum)+32e-12*max(1.,r)
        if error<=budget:
            return tuple(tuple(tuple(v*r for v in point) for point in tri) for tri in triangles),math.nextafter(error,math.inf)
        if len(triangles)*4>1000000:raise ValueError('analytic sphere proxy work limit')
        refined=[]
        for a,b,c in triangles:
            ab,bc,ca=[unit(tuple(p[i]+q[i] for i in range(3))) for p,q in ((a,b),(b,c),(c,a))]
            refined.extend(((a,ab,ca),(ab,b,bc),(ca,bc,c),(ab,bc,ca)))
        triangles=refined


def proxy_surface(scene,identity,tolerance):
    from render_contact_check import ContactSurface,surface_from_triangles
    from render_collision_check import placed,surface
    selection=ContactSurface(scene,identity,strict_frame=False)
    if not selection.catalog:raise ValueError('class has no runtime contact body')
    instance=scene.instances[identity];geometry,_model=scene.resolve(instance)
    folder,entry=scene.collisions[geometry]
    triangles=[];worst=0.;frames=[];offset=0
    if entry.get('glb'):
        if selection.catalog[0]['runtime_built']:
            local=surface(str(safe_child(folder,entry['glb'])))
            frame=query_frame(instance);radius=max(norm(p) for p in local.vertices)
            error=frame['rotation_error_per_unit']*radius+frame['translation_error_m']
            triangles.extend(tuple(placed(p,frame) for p in tri) for tri in local.triangles)
            worst=max(worst,error);frames.append(dict(body='collisionmesh',primitive_index=None,radius_m=radius,error_m=error,**frame))
        offset=1
    for i,p in enumerate(entry.get('prims',[])):
        if not selection.catalog[offset+i]['runtime_built']:continue
        dims=tuple(f32(v) for v in p['dims']);frame=query_frame(instance,p)
        stretch=math.sqrt(sum(v*v for column in frame['rotation'] for v in column))
        # Reserve only 1/16 of the unchanged contact budget for analytic
        # approximation. A coarser proxy can leave almost no room after the
        # independent query/draw bounds, despite fitting actual contact art.
        local,error=analytic_proxy(p['kind'],dims,tolerance/(16*max(1.,stretch)))
        radius=norm(dims) if p['kind']=='box' else dims[0] if p['kind']=='sphere' else math.hypot(dims[0],dims[1]/2)
        bound=error*stretch+frame['rotation_error_per_unit']*radius+frame['translation_error_m']+1e-10*max(1.,radius)
        triangles.extend(tuple(placed(point,frame) for point in tri) for tri in local)
        worst=max(worst,bound);frames.append(dict(body=p['name'],primitive_index=i,radius_m=radius,proxy_local_error_m=error,error_m=bound,**frame))
        if len(triangles)>1000000:raise ValueError('complete active collision proxy work limit')
    result=surface_from_triangles(triangles)
    if result is None:raise ValueError('no actual active collision surface')
    return result,math.nextafter(worst,math.inf),selection.catalog,frames


def placed_draw_surfaces(source,identity,paths):
    from render_contact_check import draw_surfaces,surface_from_triangles
    from render_collision_check import placed
    potential,covered,records=draw_surfaces(source,paths)
    instance=source.instances[identity]
    frame=dict(rotation=[[f32(v) for v in c] for c in instance['rotation']],translation=[f32(v) for v in instance['translation']])
    outer=surface_from_triangles([tuple(placed(p,frame) for p in tri) for tri in potential.triangles]) if potential else None
    inner=surface_from_triangles([tuple(placed(p,frame) for p in tri) for tri in covered.triangles]) if covered else None
    extent=max(1.,*(abs(v) for p in potential.vertices for v in p)) if potential else 1.
    rows=max(sum(abs(frame['rotation'][c][r]) for c in range(3)) for r in range(3))
    translation=max(abs(v) for v in frame['translation'])
    # 32u exceeds the forward-error bound of the affine four-term draw and
    # raw-ray transforms, including fused/unfused evaluation and f32 inputs.
    guard=math.nextafter(32*(2.**-23)*(rows*extent+translation+1.),math.inf)
    return outer,inner,records,guard
