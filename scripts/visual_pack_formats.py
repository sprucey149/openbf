"""Conservative native presentation payload checks for visual_packs (stdlib only)."""
import json
import math
import struct
from pathlib import Path

KTX_MAGIC = b'\xabKTX 20\xbb\r\n\x1a\n'
# render::device requests wgpu's default 2D texture limit.
MAX_KTX_EDGE = 8192
# First demonstrated static draw mesh. No terrain, side models, attachments or skins.
STATIC_MODELS = {'bf1_rhn2/models/rhn_bldg_catacombs_wall.glb'}
# Explicit draw-only companions; stock model entries remain in the archive.
PRESENTATION_MODELS = {
    name.replace('/models/', '/presentation/models/', 1): name
    for name in json.loads((Path(__file__).resolve().parents[1] /
                           'engine/crates/game/src/assets/presentation_models.json').read_text())
}


def json_object(raw):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f'duplicate JSON key {key!r}')
            result[key] = value
        return result
    def invalid_number(value):
        raise ValueError(f'invalid JSON number {value}')
    return json.loads(raw, object_pairs_hook=unique, parse_constant=invalid_number)


def ktx2(raw):
    """Accept the runtime's RGBA8 sRGB, 2D, full-chain native bake format."""
    if len(raw) < 80 or raw[:12] != KTX_MAGIC:
        raise ValueError('invalid KTX2 header')
    fmt, unit, w, h, depth, layers, faces, levels, compression = struct.unpack_from('<9I', raw, 12)
    if (fmt, unit, depth, layers, faces, compression) != (43, 1, 0, 0, 1, 0):
        raise ValueError('KTX2 must be uncompressed 2D RGBA8 sRGB (vkFormat 43), one face, no layers')
    if not (0 < w <= MAX_KTX_EDGE and 0 < h <= MAX_KTX_EDGE):
        raise ValueError(f'KTX2 dimensions must be between 1 and {MAX_KTX_EDGE} pixels')
    if levels != max(w, h).bit_length():
        raise ValueError('KTX2 needs a complete mip chain')
    doff, dlen, koff, klen, soff, slen = struct.unpack_from('<4I2Q', raw, 48)
    end = 80 + levels * 24
    if end > len(raw) or dlen != 92 or doff < end or doff + dlen > len(raw):
        raise ValueError('invalid KTX2 data format descriptor')
    dfd = raw[doff:doff+dlen]
    if (struct.unpack_from('<IIHH', dfd) != (92, 0, 2, 88)
            or dfd[12:28] != bytes([1, 1, 2, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0])):
        raise ValueError('KTX2 must describe straight-alpha RGBA8 sRGB / BT709')
    for i, channel in enumerate((0, 1, 2, 15)):
        if dfd[28+i*16:44+i*16] != struct.pack('<HBB4BII', i*8, 7, channel, 0, 0, 0, 0, 0, 255):
            raise ValueError('unsupported KTX2 channel descriptor')
    if klen or slen or soff or koff not in (0, doff+dlen):
        raise ValueError('KTX2 orientation/swizzle metadata or supercompression is unsupported; use native bake layout')
    spans = [(0, end), (doff, doff+dlen)]
    alpha = []
    for level in range(levels):
        off, length, unpacked = struct.unpack_from('<3Q', raw, 80+level*24)
        expected = max(1, w >> level) * max(1, h >> level) * 4
        if length != expected or unpacked != expected or off % 4 or off + length > len(raw):
            raise ValueError(f'invalid KTX2 mip {level}: expected {expected} RGBA8 bytes')
        spans.append((off, off+length))
        alpha.append(raw[off+3:off+length:4])
    ordered = sorted(spans)
    if any(a[1] > b[0] for a, b in zip(ordered, ordered[1:])):
        raise ValueError('overlapping KTX2 sections')
    return dict(size=(w, h), alpha=alpha, dfd=dfd)


def compatible_texture(stock, replacement):
    w, h = stock['size']; rw, rh = replacement['size']
    if w * rh != rw * h or stock['dfd'] != replacement['dfd']:
        raise ValueError('KTX2 replacement must preserve aspect ratio and color/alpha interpretation')
    opaque = lambda data: all(a.count(255) == len(a) for a in data['alpha'])
    if not (opaque(stock) and opaque(replacement)):
        if stock['size'] != replacement['size'] or stock['alpha'] != replacement['alpha']:
            raise ValueError('nonopaque KTX2 must preserve dimensions and every mip alpha sample')


def glb(raw):
    """Validate self-contained, rigid triangle meshes and all accessed binary ranges."""
    if len(raw) < 28 or struct.unpack_from('<4sII', raw) != (b'glTF', 2, len(raw)):
        raise ValueError('invalid GLB header')
    length, tag = struct.unpack_from('<I4s', raw, 12)
    at = 20 + length
    if tag != b'JSON' or length % 4 or at+8 > len(raw):
        raise ValueError('invalid GLB JSON chunk')
    doc = json_object(raw[20:at])
    size, tag = struct.unpack_from('<I4s', raw, at)
    binary = raw[at+8:]
    if tag != b'BIN\x00' or size != len(binary) or size % 4:
        raise ValueError('invalid GLB binary chunk')
    if not isinstance(doc, dict) or doc.get('asset', {}).get('version') != '2.0':
        raise ValueError('GLB must be glTF 2.0')
    # Runtime imports embedded images but binds textures through baked material names.
    if any(doc.get(key) for key in ('skins', 'animations', 'images', 'textures', 'samplers',
                                    'extensionsUsed', 'extensionsRequired', 'extensions')):
        raise ValueError('GLB skins, animations, embedded images and extensions are unsupported; use baked material bindings')
    buffers = doc.get('buffers', [])
    if (len(buffers) != 1 or set(buffers[0]) != {'byteLength'}
            or type(buffers[0]['byteLength']) is not int or not 0 <= size-buffers[0]['byteLength'] <= 3):
        raise ValueError('GLB must use one internal buffer without external references')
    def integer(value):
        if type(value) is not int or value < 0:
            raise ValueError('invalid GLB nonnegative integer')
        return value
    def row(rows, index):
        index = integer(index)
        if index >= len(rows):
            raise ValueError('out-of-range GLB index')
        return rows[index]
    views = doc.get('bufferViews', [])
    for view in views:
        if view.get('buffer') != 0 or 'extensions' in view:
            raise ValueError('GLB external buffers/extensions are unsupported')
        off, length = integer(view.get('byteOffset', 0)), integer(view['byteLength'])
        if off+length > buffers[0]['byteLength']:
            raise ValueError('GLB buffer view exceeds binary buffer')
    accessors = doc.get('accessors', [])
    decoded = {}
    def accessor(index):
        index = integer(index)
        if index in decoded:
            return decoded[index]
        a = row(accessors, index)
        if 'sparse' in a or 'extensions' in a:
            raise ValueError('GLB sparse/extended accessors are unsupported')
        view = row(views, a['bufferView'])
        components = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4}
        formats = {5121: 'B', 5123: 'H', 5125: 'I', 5126: 'f'}
        if a['type'] not in components or a['componentType'] not in formats:
            raise ValueError('unsupported GLB accessor type')
        fmt = '<' + formats[a['componentType']] * components[a['type']]
        width = struct.calcsize(fmt)
        stride = integer(view.get('byteStride', width)); offset = integer(a.get('byteOffset', 0))
        count = integer(a['count'])
        if not count or stride < width or offset+(count-1)*stride+width > view['byteLength']:
            raise ValueError('GLB accessor exceeds its view or is empty')
        values = [struct.unpack_from(fmt, binary, view.get('byteOffset', 0)+offset+i*stride) for i in range(count)]
        if any(not math.isfinite(v) for value in values for v in value):
            raise ValueError('nonfinite GLB vertex data')
        decoded[index] = values
        return values
    for index in range(len(accessors)):
        accessor(index)
    meshes = doc.get('meshes', [])
    if len(meshes) != 1 or not meshes[0].get('primitives'):
        raise ValueError('GLB needs one static mesh with draw primitives')
    if set(meshes[0]) - {'name', 'extras', 'primitives'}:
        raise ValueError('GLB morph targets or mesh extensions are unsupported')
    positions = []
    materials = doc.get('materials', [])
    for prim in meshes[0]['primitives']:
        if set(prim) - {'attributes', 'indices', 'material', 'mode'} or prim.get('mode', 4) != 4:
            raise ValueError('GLB requires indexed triangles without morph targets/extensions')
        attrs = prim['attributes']
        if set(attrs) - {'POSITION', 'NORMAL', 'TEXCOORD_0', 'COLOR_0'} or not {'POSITION', 'NORMAL', 'TEXCOORD_0'} <= attrs.keys():
            raise ValueError('GLB requires rigid POSITION, NORMAL and TEXCOORD_0 attributes')
        p = accessor(attrs['POSITION']); positions.extend(p)
        for key, index in attrs.items():
            a = row(accessors, index)
            expected = {'POSITION': 'VEC3', 'NORMAL': 'VEC3', 'TEXCOORD_0': 'VEC2'}
            if len(accessor(index)) != len(p) or (key in expected and (a['type'] != expected[key] or a['componentType'] != 5126 or a.get('normalized', False))):
                raise ValueError('invalid GLB attribute shape/type')
            if key == 'COLOR_0' and (a['type'] not in ('VEC3', 'VEC4') or a['componentType'] not in (5121, 5123, 5126)):
                raise ValueError('invalid GLB vertex colors')
            if key == 'COLOR_0' and a['type'] == 'VEC4':
                full = {5121: 255, 5123: 65535, 5126: 1.0}[a['componentType']]
                if any(v[3] != full for v in accessor(index)):
                    raise ValueError('static GLB requires opaque vertex alpha')
        ix = row(accessors, prim['indices'])
        indices = accessor(prim['indices'])
        if ix['type'] != 'SCALAR' or ix['componentType'] not in (5121, 5123, 5125) or ix.get('normalized', False) or len(indices) % 3 or any(v[0] >= len(p) for v in indices):
            raise ValueError('invalid GLB triangle indices')
        mat = row(materials, prim['material'])
        if mat.get('alphaMode', 'OPAQUE') != 'OPAQUE' or not mat.get('name'):
            raise ValueError('first static GLB scope requires named opaque material bindings')
    return dict(doc=doc, bounds=[[min(p[i] for p in positions) for i in range(3)],
                                  [max(p[i] for p in positions) for i in range(3)]])


def compatible_mesh(stock, replacement):
    a, b = stock['doc'], replacement['doc']
    for key in ('nodes', 'scenes', 'scene', 'materials'):
        if a.get(key) != b.get(key):
            raise ValueError(f'GLB must preserve baked {key} exactly (including material names and render extras)')
    am, bm = a['meshes'][0], b['meshes'][0]
    if am.get('extras') != bm.get('extras') or am.get('name') != bm.get('name'):
        raise ValueError('GLB must preserve mesh name and gameplay/culling extras (sphere, vis_top)')
    if len(am['primitives']) != len(bm['primitives']) or any(x['material'] != y['material'] for x, y in zip(am['primitives'], bm['primitives'])):
        raise ValueError('GLB must preserve ordered primitive material bindings')
    if am.get('extras', {}).get('contact_surface') or any(k.startswith('render_grid') for k in am.get('extras', {})):
        raise ValueError('GLB contact surfaces and render terrain are excluded')
    if any(abs(x-y) > 1e-6 for ax, bx in zip(stock['bounds'], replacement['bounds']) for x, y in zip(ax, bx)):
        raise ValueError('static GLB must preserve original bounds within 0.000001 metres')
