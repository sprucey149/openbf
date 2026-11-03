"""Outward f32 intervals for Body::isometry_of and composed body frames.

The port normalizes columns, converts them with glam's four rotation branches,
normalizes that quaternion, then nalgebra normalizes it again. Raw-matrix ray
queries and normalized-isometry contacts are both enclosed. Each interval
operation includes a full f32 ULP outward; sum_all covers every association and
fused multiply/add (unrounded products are also inside mul's interval).
"""
from functools import lru_cache
import math
import struct


def f32(value):
    result=struct.unpack('<f',struct.pack('<f',value))[0]
    if not math.isfinite(result): raise ValueError('nonfinite f32 query frame')
    return result


def adjacent(value,direction):
    value=f32(value)
    if value==0: return math.copysign(2.**-149,direction)
    bits=struct.unpack('<I',struct.pack('<f',value))[0]
    bits += 1 if (value>0)==(direction>0) else -1
    result=struct.unpack('<f',struct.pack('<I',bits))[0]
    if not math.isfinite(result):raise ValueError('query frame interval overflow')
    return result


def outward(lo,hi):
    return adjacent(f32(lo),-1),adjacent(f32(hi),1)


def iv(value):
    v=f32(value);return v,v


def add(a,b):return outward(a[0]+b[0],a[1]+b[1])
def neg(a):return -a[1],-a[0]
def sub(a,b):return add(a,neg(b))
def mul(a,b):
    values=[x*y for x in a for y in b];return outward(min(values),max(values))
def div(a,b):
    if b[0]<=0<=b[1]:raise ValueError('query frame interval division through zero')
    values=[x/y for x in a for y in b];return outward(min(values),max(values))
def sqrt(a):
    if a[0]<0:raise ValueError('negative query frame squared length')
    return outward(math.sqrt(a[0]),math.sqrt(a[1]))
def union(values):return min(v[0] for v in values),max(v[1] for v in values)


def sum_all(values):
    """Every binary association, not one presumed SIMD reduction order."""
    @lru_cache(None)
    def part(mask):
        indices=[i for i in range(len(values)) if mask&(1<<i)]
        if len(indices)==1:return values[indices[0]]
        first=1<<indices[0];choices=[];subset=(mask-1)&mask
        while subset:
            if subset&first and subset!=mask:choices.append(add(part(subset),part(mask^subset)))
            subset=(subset-1)&mask
        return union(choices)
    return part((1<<len(values))-1)


def square(value):
    lo,hi=value
    lower=0. if lo<=0<=hi else min(lo*lo,hi*hi)
    result=outward(lower,max(lo*lo,hi*hi))
    return max(0.,result[0]),result[1]


def normalize(vector):
    length=sqrt(sum_all([square(v) for v in vector]))
    inverse=div(iv(1.),length)
    return [mul(v,inverse) for v in vector]


def quaternion(columns):
    x,y,z=[normalize(c) for c in columns]
    m00,m01,m02=x;m10,m11,m12=y;m20,m21,m22=z
    one,half=iv(1.),iv(.5);results=[]
    if m22[0]<=0:
        dif=sub(m11,m00);base=sub(one,m22)
        if dif[0]<=0:
            four=sub(base,dif);scale=div(half,sqrt(four))
            results.append([mul(four,scale),mul(add(m01,m10),scale),mul(add(m02,m20),scale),mul(sub(m12,m21),scale)])
        if dif[1]>0:
            four=add(base,dif);scale=div(half,sqrt(four))
            results.append([mul(add(m01,m10),scale),mul(four,scale),mul(add(m12,m21),scale),mul(sub(m20,m02),scale)])
    if m22[1]>0:
        total=add(m11,m00);base=add(one,m22)
        if total[0]<=0:
            four=sub(base,total);scale=div(half,sqrt(four))
            results.append([mul(add(m02,m20),scale),mul(add(m12,m21),scale),mul(four,scale),mul(sub(m01,m10),scale)])
        if total[1]>0:
            four=add(base,total);scale=div(half,sqrt(four))
            results.append([mul(sub(m12,m21),scale),mul(sub(m20,m02),scale),mul(sub(m01,m10),scale),mul(four,scale)])
    return [normalize(normalize(q)) for q in results]


def rotation(q):
    x,y,z,w=q;one,two=iv(1.),iv(2.)
    xx,yy,zz=[mul(v,v) for v in (x,y,z)]
    xy,xz,yz=mul(x,y),mul(x,z),mul(y,z)
    xw,yw,zw=mul(x,w),mul(y,w),mul(z,w)
    return [[sub(one,mul(two,add(yy,zz))),mul(two,add(xy,zw)),mul(two,sub(xz,yw))],
            [mul(two,sub(xy,zw)),sub(one,mul(two,add(xx,zz))),mul(two,add(yz,xw))],
            [mul(two,add(xz,yw)),mul(two,sub(yz,xw)),sub(one,mul(two,add(xx,yy)))]]


def compose(instance,primitive=None):
    a=[[f32(v) for v in c] for c in instance['rotation']];at=[f32(v) for v in instance['translation']]
    if primitive is None:
        return a,at,[[iv(v) for v in c] for c in a],[iv(v) for v in at]
    b=[[f32(v) for v in c] for c in primitive['rotation']];bt=[f32(v) for v in primitive['translation']]
    nominal=[[sum(a[k][r]*b[c][k] for k in range(3)) for r in range(3)] for c in range(3)]
    translation=[sum(a[k][r]*bt[k] for k in range(3))+at[r] for r in range(3)]
    columns=[[sum_all([mul(iv(a[k][r]),iv(b[c][k])) for k in range(3)]) for r in range(3)] for c in range(3)]
    offset=[sum_all([*(mul(iv(a[k][r]),iv(bt[k])) for k in range(3)),iv(at[r])]) for r in range(3)]
    return nominal,translation,columns,offset


def query_frame(instance,primitive=None):
    nominal,translation,columns,offset=compose(instance,primitive)
    gram=[[sum(nominal[i][k]*nominal[j][k] for k in range(3)) for j in range(3)] for i in range(3)]
    deviation=max(sum(abs(gram[i][j]-(1. if i==j else 0.)) for j in range(3)) for i in range(3))
    deviation=math.nextafter(deviation+1e-12,math.inf)
    if deviation>=.01:
        raise ValueError('unsupported non-rigid or ill-conditioned collision query frame')
    quaternions=quaternion(columns)
    norm_upper=0.
    for q in quaternions:
        squared=(0.,0.)
        for value in q:
            magnitude=max(abs(value[0]),abs(value[1]))
            squared=real_add(squared,real_mul((magnitude,magnitude),(magnitude,magnitude)))
        norm_upper=max(norm_upper,math.nextafter(math.sqrt(squared[1]),math.inf))
    if norm_upper>1.001:
        raise ValueError('quaternion coefficient norm exceeds the proved roundoff domain')
    rotations=[rotation(q) for q in quaternions]
    ray_columns,ray_offset,stored_inverse=ray_frame(columns,offset)
    frames=[columns,ray_columns,*rotations]
    offset=[union([offset[i],ray_offset[i]]) for i in range(3)]
    # Frobenius bounds the Euclidean operator norm, so this covers every
    # point of a mesh or primitive with a known local radius.
    squared=(0.,0.)
    for c in range(3):
        for r in range(3):
            maximum=math.nextafter(max(abs(frame[c][r][end]-nominal[c][r]) for frame in frames for end in (0,1)),math.inf)
            squared=real_add(squared,real_mul((maximum,maximum),(maximum,maximum)))
    error=math.nextafter(math.sqrt(squared[1]),math.inf)
    squared=(0.,0.)
    for i in range(3):
        maximum=math.nextafter(max(abs(v-translation[i]) for v in offset[i]),math.inf)
        squared=real_add(squared,real_mul((maximum,maximum),(maximum,maximum)))
    offset_error=math.nextafter(math.sqrt(squared[1]),math.inf)
    # nalgebra 0.33.3 quaternion_ops computes t=2*(q_xyz cross v),
    # c=q_xyz cross t, then t*w+c+v. With u=2^-24 and
    # gamma_3=3u/(1-3u), one cross has norm error <=sqrt(2)*gamma_3
    # times |a|*|b|. For a=|q|<=1.001 this gives, in units u*|v|:
    # first cross and propagation through c: <17.1;
    # propagation through t*w and its product: <10.6;
    # the two final vector additions: <9.1. The total is <36.8<40.
    # The interval coefficient norm is checked above, not presumed unit.
    # Multiplication by two is exact except subnormals, whose absolute
    # error is dominated by the existing extent>=1 draw/proxy guards.
    # 64*f32_epsilon =128u therefore gives more than a threefold envelope.
    # The coefficient intervals above also contain the ideal polynomial,
    # so this bounds its actual f32 evaluation, not an assumed exact unit q.
    error+=64*(2.**-23)
    return dict(rotation=nominal,translation=translation,rotation_error_per_unit=math.nextafter(error,math.inf),
                translation_error_m=math.nextafter(offset_error,math.inf),
                quaternion_norm_upper=norm_upper,gram_deviation_upper=deviation,
                stored_inverse_intervals=stored_inverse,ray_frame_included=True,
                method='outward_f32_all_query_frame_intervals_v2')


def det3(matrix, rounded=True):
    """Enclose each row/column cofactor expansion used by scalar/SIMD inverse."""
    plus,times,minus=(add,mul,sub) if rounded else (real_add,real_mul,real_sub)
    variants=[]
    for transpose in (False,True):
        rows=list(map(list,zip(*matrix))) if transpose else matrix
        for row in range(3):
            other=[i for i in range(3) if i!=row];terms=[]
            for column in range(3):
                cols=[i for i in range(3) if i!=column]
                minor=minus(times(rows[other[0]][cols[0]],rows[other[1]][cols[1]]),
                            times(rows[other[0]][cols[1]],rows[other[1]][cols[0]]))
                value=times(rows[row][column],minor)
                terms.append(neg(value) if (row+column)%2 else value)
            variants.append(sum_all(terms) if rounded else real_add(real_add(terms[0],terms[1]),terms[2]))
    return union(variants)


def real_out(lo,hi):return math.nextafter(lo,-math.inf),math.nextafter(hi,math.inf)
def real_add(a,b):return real_out(a[0]+b[0],a[1]+b[1])
def real_sub(a,b):return real_add(a,neg(b))
def real_mul(a,b):
    values=[x*y for x in a for y in b];return real_out(min(values),max(values))
def real_div(a,b):
    if b[0]<=0<=b[1]:raise ValueError('ill-conditioned inverse query interval')
    values=[x/y for x in a for y in b];return real_out(min(values),max(values))


def ray_frame(columns,offset):
    """Effective affine ray surface inverse(stored f32 Mat4::inverse(M)).

    The CPU transforms origins/directions with the upper three rows of its
    stored inverse (no homogeneous divide). Its f32 cofactor/determinant inverse
    is enclosed first. We then invert that interval affine matrix in real,
    outward-rounded f64 arithmetic to bound the corresponding world surface.
    """
    matrix=[[columns[c][r] for c in range(3)]+[offset[r]] for r in range(3)]
    matrix.append([iv(0.),iv(0.),iv(0.),iv(1.)])
    cofactors=[]
    for row in range(4):
        cofactors.append([])
        for col in range(4):
            minor=[[matrix[r][c] for c in range(4) if c!=col] for r in range(4) if r!=row]
            value=det3(minor)
            cofactors[row].append(neg(value) if (row+col)%2 else value)
    determinants=[]
    for col in range(4):
        determinants.append(sum_all([mul(matrix[r][col],cofactors[r][col]) for r in range(4)]))
    determinant=union(determinants)
    reciprocal=div(iv(1.),determinant)
    stored=[[mul(cofactors[c][r],reciprocal) for c in range(4)] for r in range(4)]
    linear=[row[:3] for row in stored[:3]];translation=[row[3] for row in stored[:3]]
    determinant=det3(linear,rounded=False)
    inverse=[]
    for row in range(3):
        inverse.append([])
        for col in range(3):
            rows=[i for i in range(3) if i!=col];cols=[i for i in range(3) if i!=row]
            value=real_sub(real_mul(linear[rows[0]][cols[0]],linear[rows[1]][cols[1]]),
                           real_mul(linear[rows[0]][cols[1]],linear[rows[1]][cols[0]]))
            if (row+col)%2:value=neg(value)
            inverse[row].append(real_div(value,determinant))
    centre=[neg(real_add(real_add(real_mul(row[0],translation[0]),real_mul(row[1],translation[1])),
                        real_mul(row[2],translation[2]))) for row in inverse]
    return [[inverse[r][c] for r in range(3)] for c in range(3)],centre,stored
