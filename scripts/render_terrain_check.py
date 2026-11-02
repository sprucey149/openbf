#!/usr/bin/env python3
"""Exhaustive terrain facet checks against the PS2 query grid.

Every indexed base, painted-layer and far-sheet triangle is partitioned at every
canonical cell and diagonal it crosses. Heights on each resulting polygon are
affine, so their extrema occur at its vertices; this is not a point sample.
Base/far coverage is checked by exact polygon differences per cell triangle,
then a continuous distance bound for every missing or excess fragment.
Reports contain counts and numerical witnesses, never the terrain geometry.
"""
from functools import lru_cache
import hashlib
import math
from pathlib import Path

from world_fingerprint import glb_geometry, read_ron, safe_child
from render_terrain_dependencies import terrain_dependencies, dependency_problems

MARGIN_M = 0.01  # 1 cm covers ordinary patch quantization and float32 rounding; authored PS2 exceptions are reported.


def area(poly):
    return abs(sum(a[0] * b[2] - a[2] * b[0]
                   for a, b in zip(poly, poly[1:] + poly[:1]))) * 0.5


def split(poly, plane):
    def distance(p):
        return sum(a * b for a, b in zip(plane[:3], p)) + plane[3]
    values = [distance(p) for p in poly]
    if all(v >= 0 for v in values):
        return [], poly
    if all(v <= 0 for v in values):
        return poly, []
    inside, outside = [], []
    for i, a in enumerate(poly):
        j = (i + 1) % len(poly)
        b, da, db = poly[j], values[i], values[j]
        if da <= 0:
            inside.append(a)
        if da >= 0:
            outside.append(a)
        if (da < 0 < db) or (db < 0 < da):
            f = da / (da - db)
            p = [a[k] + (b[k] - a[k]) * f for k in range(3)]
            inside.append(p); outside.append(p)
    return inside, outside


def subtract(poly, cutters):
    polys = [poly]
    for cutter in cutters:
        following = []
        for poly in polys:
            remaining = poly
            for plane in cutter['planes']:
                if len(remaining) < 3:
                    break
                remaining, outside = split(remaining, plane)
                if len(outside) >= 3 and area(outside) > 0:
                    following.append(outside)
        polys = following
    return polys


def clip_triangle(poly, tri):
    # Canonical triangle winding is clockwise when seen in world X/Z.
    orientation = sum(a[0] * b[2] - a[2] * b[0] for a, b in zip(tri, tri[1:] + tri[:1]))
    sign = 1 if orientation > 0 else -1
    for a, b in zip(tri, tri[1:] + tri[:1]):
        # Inside is the side of this directed edge carrying the third vertex.
        plane = [sign * (b[2] - a[2]), 0, -sign * (b[0] - a[0]),
                 sign * ((b[0] - a[0]) * a[2] - (b[2] - a[2]) * a[0])]
        poly, _outside = split(poly, plane)
        if len(poly) < 3:
            return []
    return poly


def difference(polys, cutters):
    """Subtract convex X/Z polygons, preserving convex fragments in 3-D."""
    for cutter in cutters:
        orientation=sum(a[0]*b[2]-a[2]*b[0] for a,b in zip(cutter,cutter[1:]+cutter[:1]))
        if orientation==0:continue
        sign=1 if orientation>0 else -1
        following=[]
        for poly in polys:
            remaining=poly
            for a,b in zip(cutter,cutter[1:]+cutter[:1]):
                if len(remaining)<3:break
                plane_=[sign*(b[2]-a[2]),0,-sign*(b[0]-a[0]),
                        sign*((b[0]-a[0])*a[2]-(b[2]-a[2])*a[0])]
                remaining,outside=split(remaining,plane_)
                if len(outside)>=3 and area(outside)>0:following.append(outside)
        polys=following
    return polys


def triangles(polys):
    return [[poly[0],poly[i],poly[i+1]] for poly in polys for i in range(1,len(poly)-1)
            if area([poly[0],poly[i],poly[i+1]])>0]


def coverage_certificate(polys, candidates, margin=MARGIN_M):
    """An upper bound for every point, using convex distance and subdivision.

    Distance to ONE convex triangle is convex. Its maximum over a convex
    polygon is bounded by its values at every vertex. Taking the best such
    triangle gives a conservative upper bound on distance to the union. If
    needed, partition the polygon; no sampled point can produce a pass.
    """
    from render_collision_check import triangle_distance2
    pending=[(triangle,0) for triangle in triangles(polys)]
    steps,worst,at=0,0.0,None
    if pending and not candidates:
        return {'complete':False,'upper_bound_m':None,'steps':0,'witness':pending[0][0][0],
                'reason':'no neighbouring surface can certify terrain coverage'}
    while pending:
        poly,depth=pending.pop();steps+=1
        bound=min(max(triangle_distance2(p,tri) for p in poly) for tri in candidates)
        if bound<=margin*margin:
            if math.sqrt(bound)>worst:worst,at=math.sqrt(bound),poly[0]
            continue
        if any(min(triangle_distance2(p,tri) for tri in candidates)>margin*margin for p in poly):
            return {'complete':False,'upper_bound_m':None,'steps':steps,'witness':poly[0],
                    'reason':'terrain coverage cannot meet the margin using neighbouring faces'}
        if depth>=40 or steps>=100000:
            return {'complete':False,'upper_bound_m':None,'steps':steps,'witness':poly[0],
                    'reason':'continuous terrain coverage exceeds the proof budget'}
        # Split the longest edge. This preserves skinny seam fragments and
        # converges without spreading a fixed sampling grid over the scene.
        edge=max(range(3),key=lambda i:sum((poly[i][k]-poly[(i+1)%3][k])**2 for k in range(3)))
        a,b,c=poly[edge],poly[(edge+1)%3],poly[(edge+2)%3]
        mid=[(a[i]+b[i])*0.5 for i in range(3)]
        pending.append(([a,mid,c],depth+1));pending.append(([mid,b,c],depth+1))
    return {'complete':True,'upper_bound_m':worst,'steps':steps,'witness':at}


class Grid:
    def __init__(self, positions, query_size=None):
        if not positions:
            raise ValueError('empty terrain query grid')
        self.n = next((i for i, p in enumerate(positions[1:], 1) if p[0] == positions[0][0]), 0)
        if self.n < 2 or self.n * self.n > len(positions):
            raise ValueError('terrain lacks complete query grid')
        self.unit = positions[1][0] - positions[0][0]
        self.x0, self.z0 = positions[0][0], positions[0][2]
        if not math.isfinite(self.unit) or self.unit <= 0:
            raise ValueError('invalid terrain grid spacing')
        self.heights = []
        self._triangles = {}
        self._details = {}
        for z in range(self.n):
            for x in range(self.n):
                p = positions[z * self.n + x]
                if not all(math.isfinite(v) for v in p) or p[0] != self.x0 + x * self.unit or p[2] != self.z0 + z * self.unit:
                    raise ValueError('nonuniform terrain query grid')
                self.heights.append(p[1])
        if query_size is not None:
            if type(query_size) is not int or not 2 <= query_size <= self.n or (self.n - query_size) % 2:
                raise ValueError('invalid canonical terrain window')
            start = (self.n - query_size) // 2
            self.heights = [self.heights[(z + start) * self.n + x + start]
                            for z in range(query_size) for x in range(query_size)]
            self.n = query_size; self.x0 += start * self.unit; self.z0 += start * self.unit

    def point(self, x, z):
        return [self.x0 + x * self.unit,
                self.heights[min(max(z, 0), self.n - 1) * self.n + min(max(x, 0), self.n - 1)],
                self.z0 + z * self.unit]

    def triangle(self, key):
        if key not in self._triangles:
            x, z, half = key
            a,b,c,d = self.point(x,z),self.point(x+1,z),self.point(x,z+1),self.point(x+1,z+1)
            self._triangles[key] = ([a,c,d],[a,d,b])[half] if z % 2 == 0 else ([a,c,b],[b,c,d])[half]
        return self._triangles[key]

    def details(self,key,cutters):
        if key not in self._details:
            tri=self.triangle(key)
            relevant=[]
            for cutter in cutters:
                if not cutter['planes']:
                    raise ValueError('empty canonical terrain cutter')
                if any(all(sum(a*b for a,b in zip(p[:3],v))+p[3]>=0 for v in tri) for p in cutter['planes']):
                    continue
                relevant.append(cutter)
            expected=subtract(tri,relevant)
            self._details[key]=(plane(tri),relevant,expected)
        return self._details[key]


def plane(tri):
    a,b,c = tri
    determinant = (b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2])
    if determinant == 0:
        return None
    x = ((b[1] - a[1]) * (c[2] - a[2]) - (c[1] - a[1]) * (b[2] - a[2])) / determinant
    z = ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])) / determinant
    return x,z,a[1] - x*a[0] - z*a[2]


def height(p, equation):
    return equation[0]*p[0] + equation[1]*p[2] + equation[2]


def partitions(tri, grid):
    xs = [(p[0] - grid.x0) / grid.unit for p in tri]
    zs = [(p[2] - grid.z0) / grid.unit for p in tri]
    # A canonical grid triangle is by far the common path. Its centroid
    # identifies the cell unambiguously, even when its corners are on edges.
    x,z = math.floor(sum(xs)/3),math.floor(sum(zs)/3)
    if min(xs) >= x and max(xs) <= x+1 and min(zs) >= z and max(zs) <= z+1:
        side = [(xx-x)-(zz-z) if z%2==0 else (xx-x)+(zz-z)-1 for xx,zz in zip(xs,zs)]
        if max(side) <= 0 or min(side) >= 0:
            yield (x,z,0 if max(side)<=0 else 1),tri
            return
    for z in range(math.floor(min(zs)),math.ceil(max(zs))):
        for x in range(math.floor(min(xs)),math.ceil(max(xs))):
            for half in (0,1):
                key=x,z,half
                poly=clip_triangle(tri,grid.triangle(key))
                if len(poly)>=3 and area(poly)>0:
                    yield key,poly


def nonempty_3d(poly):
    if len(poly)<3:return False
    a=poly[0]
    for b,c in zip(poly[1:],poly[2:]):
        u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
        if any(x!=0 for x in (u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0])):
            return True
    return False


def vertical_bound(tri,grid,cutters):
    """Continuous 3-D upper bound for a quantized vertical reference facet.

    Grid and diagonal planes partition the complete 3-D triangle. Distance to
    one canonical convex face is convex, so its values at all fragment vertices
    bound the whole fragment. These faces are baseline cases, never discarded.
    A non-identical donor vertical remains fail-closed separately.
    """
    from render_collision_check import triangle_distance2
    xs=[(p[0]-grid.x0)/grid.unit for p in tri]
    zs=[(p[2]-grid.z0)/grid.unit for p in tri]
    planes=[[1,0,0,-(grid.x0+x*grid.unit)] for x in range(math.floor(min(xs))+1,math.ceil(max(xs)))]
    planes += [[0,0,1,-(grid.z0+z*grid.unit)] for z in range(math.floor(min(zs))+1,math.ceil(max(zs)))]
    polys=[tri]
    for plane_ in planes:
        polys=[part for poly in polys for part in split(poly,plane_) if nonempty_3d(part)]
    worst,at,fragments=0.0,None,0
    for poly in polys:
        centre=[sum(p[i] for p in poly)/len(poly) for i in range(3)]
        x=math.floor((centre[0]-grid.x0)/grid.unit);z=math.floor((centre[2]-grid.z0)/grid.unit)
        wx,wz=grid.x0+x*grid.unit,grid.z0+z*grid.unit
        diagonal=[1,0,-1,wz-wx] if z%2==0 else [1,0,1,-wx-wz-grid.unit]
        for part in split(poly,diagonal):
            if not nonempty_3d(part):continue
            fragments+=1
            candidates=[face for zz in range(z-1,z+2) for xx in range(x-1,x+2) for half in (0,1)
                        for face in triangles(grid.details((xx,zz,half),cutters)[2])]
            if not candidates:
                return {'complete':False,'upper_bound_m':None,'witness':part[0],'fragments':fragments}
            best=min((max(triangle_distance2(p,face) for p in part),face) for face in candidates)
            bound=math.sqrt(best[0])
            if bound>worst:
                worst=bound;at=max(part,key=lambda p:triangle_distance2(p,best[1]))
    return {'complete':True,'upper_bound_m':worst,'witness':at,'fragments':fragments}


def measure(path, grid, cutters, coverage, coverage_margin=MARGIN_M):
    positions,indices = glb_geometry(path)
    if not positions or not indices or len(indices)%3 or any(not all(math.isfinite(v) for v in p) for p in positions):
        raise ValueError('invalid indexed terrain surface')
    worst,at,vertical,degenerate,cells,projected,actual_faces = 0.0,None,0,0,{},{},{}
    vertical_complete,vertical_upper,vertical_at=True,0.0,None
    vertical_cases=[]
    for first in range(0,len(indices),3):
        tri=[positions[i] for i in indices[first:first+3]]
        equation=plane(tri)
        if equation is None:
            a,b,c=tri
            u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
            cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
            if all(component==0 for component in cross):
                degenerate+=1  # An empty face has no drawn surface to measure.
            else:
                vertical+=1
                proof=vertical_bound(tri,grid,cutters)
                vertical_cases.append({'face':first//3,**proof})
                vertical_complete &= proof['complete']
                if proof['complete'] and proof['upper_bound_m']>vertical_upper:
                    vertical_upper=proof['upper_bound_m'];vertical_at=proof['witness']
            continue
        for key,poly in partitions(tri,grid):
            target,relevant,_expected=grid.details(key,cutters)
            target_poly=[[p[0],height(p,target),p[2]] for p in poly]
            surviving=subtract(target_poly,relevant)
            # A source face can lie over a canonical hole. Its area is retained
            # below and cannot disappear from the coverage comparison.
            if coverage:
                projected.setdefault(key,[]).append(target_poly)
                actual_faces.setdefault(key,[]).extend(triangles([poly]))
            for fragment in surviving:
                for p in fragment:
                    gap=abs(height(p,equation)-p[1])
                    if gap>worst:worst,at=gap,[p[0],height(p,equation),p[2]]
                    if gap>cells.get(key,0.0):cells[key]=gap
    missing,overcut,worst_area,area_at=0.0,0.0,0.0,None
    coverage_complete,coverage_upper,coverage_steps,coverage_at,coverage_reason=True,0.0,0,None,None
    if coverage:
        keys=set(projected)|{(x,z,half) for z in range(grid.n-1) for x in range(grid.n-1) for half in (0,1)}
        for key in keys:
            equation,relevant,expected=grid.details(key,cutters)
            actual=projected.get(key,[])
            # Exact polygon differences identify every uncovered and excess
            # facet. There is no area budget that can hide a narrow offender.
            absent=difference(expected,actual)
            extra=difference(actual,expected)
            missing_area=sum(area(p) for p in absent)
            overcut_area=sum(area(p) for p in extra)
            missing+=missing_area;overcut+=overcut_area
            if max(missing_area,overcut_area)>worst_area:
                worst_area=max(missing_area,overcut_area);area_at=grid.triangle(key)[0]
            if not absent and not extra:continue
            x,z,_half=key
            neighbours=[(xx,zz,h) for zz in range(z-1,z+2) for xx in range(x-1,x+2) for h in (0,1)]
            source_candidates=[tri for k in neighbours for tri in actual_faces.get(k,[])]
            canonical_candidates=[tri for k in neighbours for tri in triangles(grid.details(k,cutters)[2])]
            for pieces,candidates,is_extra in ((absent,source_candidates,False),(extra,canonical_candidates,True)):
                if is_extra:
                    # Restore the source face's own height for a conservative
                    # render-to-collision distance, including vertical error.
                    lifted=[]
                    for fragment in pieces:
                        for face in actual_faces.get(key,[]):
                            clipped=clip_triangle(fragment,face)
                            if len(clipped)>=3 and area(clipped)>0:
                                source_plane=plane(face)
                                lifted.append([[p[0],height(p,source_plane),p[2]] for p in clipped])
                    pieces=lifted
                proof=coverage_certificate(pieces,candidates,coverage_margin)
                coverage_steps+=proof['steps']
                if not proof['complete']:
                    coverage_complete=False;coverage_at=proof['witness'];coverage_reason=proof['reason']
                elif proof['upper_bound_m']>coverage_upper:
                    coverage_upper=proof['upper_bound_m'];coverage_at=proof['witness']
    return {'triangles':len(indices)//3,'vertical_triangles':vertical,'degenerate_triangles':degenerate,
            'vertical_complete':vertical_complete,'vertical_upper_bound_m':vertical_upper,
            'vertical_cases':vertical_cases,'height_worst_m':worst,
            'worst_m':max(worst,vertical_upper),'worst_at':vertical_at if vertical_upper>worst else at,'missing_area_m2':missing,'overcut_area_m2':overcut,
            'worst_cell_area_m2':worst_area,'area_witness':area_at,'coverage_complete':coverage_complete,
            'coverage_upper_bound_m':coverage_upper if coverage_complete else None,
            'coverage_steps':coverage_steps,'coverage_witness':coverage_at,'coverage_reason':coverage_reason},cells


@lru_cache(maxsize=4)
def reference_data(folder, modified):
    del modified
    folder=Path(folder);manifest=read_ron(folder/'manifest.ron');name=manifest.get('terrain_glb')
    if not name:return manifest,None,{}
    positions,_=glb_geometry(safe_child(folder,name))
    grid=Grid(positions,manifest.get('terrain_query_size'))
    measures={}
    for label,field in (('terrain_base','terrain_glb'),('terrain_layers','terrain_layers_glb'),('terrain_far','lowres_glb')):
        if manifest.get(field):
            measures[label]=measure(safe_child(folder,manifest[field]),grid,manifest.get('terrain_cutters',[]),label!='terrain_layers',math.inf)
    return manifest,grid,measures


def terrain_map(reference,source,key):
    receipt={'schema':1,'status':'pass','complete':True,'reference':{'kind':'absent'},'surfaces':[],
             'margin_m':MARGIN_M,'worst_excess_m':0.0,'worst_at':None,'absent_painted_layers':False}
    receipt['dependencies']={'schema':1,'status':'unmeasured','complete':False,'tiers':['ps2','pc'],
                             'records':[{'kind':'policy','status':'unmeasured','complete':False,
                                         'reason':'terrain inputs were not read','tier':None,
                                         'reference':None,'source':None}]}
    try:
        rf,sf=Path(reference)/key,Path(source)/key
        reference_manifest=read_ron(rf/'manifest.ron')
        signature=tuple(hashlib.sha256(safe_child(rf,reference_manifest[f]).read_bytes()).hexdigest() for f in ('terrain_glb','terrain_layers_glb','lowres_glb') if reference_manifest.get(f))
        rm,grid,baseline=reference_data(str(rf),((rf/'manifest.ron').stat().st_mtime_ns,signature))
        sm=read_ron(sf/'manifest.ron')
        dependencies=terrain_dependencies(reference,source,key,rm,sm)
        receipt['dependencies']=dependencies
        receipt['status']=dependencies['status']
        receipt['complete']=dependencies['complete']
        if grid is None:
            if any(sm.get(f) for f in ('terrain_glb','terrain_layers_glb','lowres_glb')):
                raise ValueError('source draws terrain without a PS2 reference heightfield')
            return receipt
        receipt['reference']={'kind':'heightfield','size':grid.n,'unit':grid.unit,'cells':(grid.n-1)**2}
        for label,field in (('terrain_base','terrain_glb'),('terrain_layers','terrain_layers_glb'),('terrain_far','lowres_glb')):
            if not sm.get(field):
                if label=='terrain_layers':
                    # These are paint passes on the already complete base,
                    # not independent ground. Missing donor blend weights
                    # must never be replaced with invented opaque paint.
                    receipt['absent_painted_layers']=label in baseline
                    continue
                if label in baseline:
                    raise ValueError(f'missing source {label}')
                continue
            reference_name=rm.get(field)
            identical=bool(reference_name) and hashlib.sha256(safe_child(sf,sm[field]).read_bytes()).digest()==hashlib.sha256(safe_child(rf,reference_name).read_bytes()).digest()
            control,control_cells=baseline.get(label,({'worst_m':0.0},{}))
            if identical:
                result,cells=dict(control),control_cells
            else:
                result,cells=measure(safe_child(sf,sm[field]),grid,rm.get('terrain_cutters',[]),label!='terrain_layers')
            excess=max(0.0,max((gap-control_cells.get(cell,0.0) for cell,gap in cells.items()),default=0.0))
            result.update({'name':label,'complete':result['vertical_triangles']==0 or (identical and result['vertical_complete']),'status':'pass',
                           'baseline_m':control['worst_m'],'excess_m':excess,
                           'baseline_missing_area_m2':control.get('missing_area_m2',0.0),
                           'baseline_overcut_area_m2':control.get('overcut_area_m2',0.0)})
            if not result['complete']:result['status']='unmeasured'
            result['reference_bytes_identical']=identical
            if not identical:
                # Donor terrain must draw at the PS2 heights everywhere,
                # independently of the reference art's own residual elsewhere.
                if result['height_worst_m']>MARGIN_M:
                    result['status']='offender'
                if not result['coverage_complete']:
                    result['complete']=False
                    if result['status']=='pass':result['status']='unmeasured'
            if result['status']=='offender' or result['status']=='unmeasured' and receipt['status']=='pass':
                receipt['status']=result['status']
            receipt['complete'] &= result['complete']
            if excess>receipt['worst_excess_m']:
                receipt['worst_excess_m']=excess;receipt['worst_at']=result['worst_at']
            receipt['surfaces'].append(result)
    except (ValueError,KeyError,IndexError,OSError,OverflowError,ZeroDivisionError) as exc:
        receipt.update(status='unmeasured',complete=False,error=str(exc))
    return receipt


def terrain_problems(receipt):
    errors=[]
    if not isinstance(receipt,dict) or receipt.get('schema')!=1 or type(receipt.get('complete'))is not bool:
        return ['invalid terrain receipt schema']
    if receipt.get('status')not in ('pass','offender','unmeasured') or not isinstance(receipt.get('reference'),dict):
        errors.append('invalid terrain receipt status/reference')
    if receipt.get('status')=='pass' and not receipt['complete']:
        errors.append('passing terrain receipt is incomplete')
    dependencies=receipt.get('dependencies')
    errors.extend(f'terrain render dependencies: {problem}' for problem in dependency_problems(dependencies))
    if receipt.get('status')=='pass' and (not isinstance(dependencies,dict)
            or dependencies.get('status')!='pass' or dependencies.get('complete')is not True):
        errors.append('passing terrain receipt has unproven render dependencies')
    if receipt.get('margin_m')!=MARGIN_M:errors.append('unexpected terrain margin')
    if type(receipt.get('worst_excess_m'))not in (int,float) or not math.isfinite(receipt['worst_excess_m']) or receipt['worst_excess_m']<0:
        errors.append('invalid terrain worst excess')
    surfaces=receipt.get('surfaces')
    if not isinstance(surfaces,list):return errors+['missing terrain surfaces']
    names=set()
    for row in surfaces:
        if (not isinstance(row,dict) or row.get('name')not in ('terrain_base','terrain_layers','terrain_far')
                or type(row.get('complete'))is not bool or row.get('status')not in ('pass','offender','unmeasured')
                or type(row.get('triangles'))is not int or row['triangles']<1):
            errors.append('invalid terrain surface record');continue
        if row['name'] in names:errors.append('duplicate terrain surface')
        names.add(row['name'])
        for key in ('worst_m','baseline_m','excess_m','missing_area_m2','overcut_area_m2'):
            if type(row.get(key))not in (int,float) or not math.isfinite(row[key]) or row[key]<0:
                errors.append(f'invalid terrain {key}')
        if type(row.get('reference_bytes_identical'))is not bool:
            errors.append('invalid terrain byte-identity flag')
        if row['status']=='pass':
            if not row['complete']:errors.append('passing terrain surface is incomplete')
            if not row.get('reference_bytes_identical'):
                if row.get('worst_m',math.inf)>MARGIN_M:errors.append('passing donor terrain exceeds height margin')
                bound=row.get('coverage_upper_bound_m')
                if row.get('coverage_complete')is not True or type(bound)not in (int,float) or not math.isfinite(bound) or not 0<=bound<=MARGIN_M:
                    errors.append('passing donor terrain lacks a continuous coverage bound')
    kind=receipt.get('reference',{}).get('kind')
    if kind not in ('absent','heightfield'):errors.append('unknown reference terrain kind')
    if kind=='absent' and surfaces:errors.append('surface without reference terrain')
    if kind=='heightfield' and receipt.get('status')=='pass' and 'terrain_base'not in names:
        errors.append('heightfield has no measured base surface')
    return errors
