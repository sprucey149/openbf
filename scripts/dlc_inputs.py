"""Content detection for the optional Kit Fisto / Asajj Ventress hero source.

The input scanner may call ``detect_dlc(path)`` on a folder, LVL or ISO9660
image. Only a structurally valid LVL containing both hero classes and a supported
geometry encoding is returned. Names of folders, images and files are immaterial.
Detection reads metadata only; it never extracts maps, scripts or other content.
"""
from dataclasses import dataclass
from pathlib import Path
import os
import struct

HERO_CLASSES = frozenset((b"dlc_hero_fisto\0", b"dlc_hero_ventress\0"))
MAX_LVL = 128 << 20
MAX_SOUND_LVL = 32 << 20


@dataclass(frozen=True)
class DlcSource:
    path: Path
    platform: str
    size: int
    member: str | None = None
    offset: int = 0

    def read_bytes(self):
        """Read only the detected side LVL, including when held inside an ISO."""
        with self.path.open("rb") as source:
            source.seek(self.offset)
            data = source.read(self.size)
        if len(data) != self.size:
            raise ValueError("DLC source changed or was truncated after detection")
        return data


def _platform(read, size):
    if size < 8 or size > MAX_LVL:
        return None
    if read(0, 8) != b"ucfb" + struct.pack("<I", size - 8):
        return None
    classes, platforms = set(), set()
    pending = [(b"ucfb", 8, size, 0)]
    count = 0
    while pending:
        parent, pos, end, depth = pending.pop()
        while pos < end:
            if end - pos < 8:
                raise ValueError("truncated LVL chunk")
            header = read(pos, 8)
            tag, length = header[:4], struct.unpack_from("<I", header, 4)[0]
            body, limit = pos + 8, pos + 8 + length
            if limit > end:
                raise ValueError("LVL chunk outside parent")
            count += 1
            if count > 65536 or depth > 16:
                raise ValueError("DLC LVL structure exceeds bounds")
            if parent == b"entc" and tag == b"TYPE" and length <= 128:
                name = read(body, length)
                if name in HERO_CLASSES:
                    classes.add(name)
            if parent == b"segm":
                if tag == b"_PS2":
                    platforms.add("ps2")
                elif tag in (b"VBUF", b"VDAT"):
                    platforms.add("pc")
            if tag == b"lvl_" and length >= 8:
                pending.append((tag, body + 8, limit, depth + 1))
            elif tag in (b"ucfb", b"modl", b"segm", b"entc"):
                pending.append((tag, body, limit, depth + 1))
            pos = (limit + 3) & ~3
    if classes == HERO_CLASSES and len(platforms) == 1:
        return platforms.pop()
    return None


def _iso_members(read, size):
    """Yield bounded ISO9660 file extents; caller inspects their content."""
    if size < 17 * 2048:
        return
    pvd = read(16 * 2048, 2048)
    if pvd[:7] != b"\x01CD001\x01":
        return
    stack = [("", struct.unpack_from("<I", pvd, 158)[0],
              struct.unpack_from("<I", pvd, 166)[0])]
    seen, total, records = set(), 0, 0
    while stack:
        prefix, lba, length = stack.pop()
        if lba in seen:
            continue
        seen.add(lba)
        total += length
        if len(seen) > 4096 or length > 4 << 20 or total > 16 << 20 or len(prefix) > 4096:
            raise ValueError("ISO directory structure exceeds bounds")
        data = read(lba * 2048, length)
        pos = 0
        while pos < len(data):
            length = data[pos]
            if not length:
                pos = (pos // 2048 + 1) * 2048
                continue
            if length < 34 or pos + length > len(data):
                raise ValueError("truncated ISO record")
            rec = data[pos:pos + length]
            pos += length
            records += 1
            if records > 65536 or rec[32] + 33 > len(rec):
                raise ValueError("invalid ISO directory record")
            name = rec[33:33 + rec[32]]
            if name in (b"\0", b"\1"):
                continue
            name = name.decode("ascii").split(";", 1)[0]
            if "/" in name or "\\" in name or name in (".", ".."):
                raise ValueError("invalid ISO member name")
            offset = struct.unpack_from("<I", rec, 2)[0] * 2048
            extent = struct.unpack_from("<I", rec, 10)[0]
            if offset + extent > size:
                raise ValueError("ISO extent outside image")
            member = prefix + name
            if rec[25] & 2:
                stack.append((member + "/", offset // 2048, extent))
            else:
                yield member, offset, extent


def _detect_file(path):
    try:
        with path.open("rb") as source:
            size = source.seek(0, 2)

            def read(offset, length):
                if offset < 0 or length < 0 or offset + length > size:
                    raise ValueError("source extent outside file")
                source.seek(offset)
                data = source.read(length)
                if len(data) != length:
                    raise ValueError("truncated source")
                return data

            platform = _platform(read, size)
            if platform:
                return [DlcSource(path, platform, size)]
            found = []
            for member, offset, extent in _iso_members(read, size):
                def member_read(pos, length):
                    if pos < 0 or pos + length > extent:
                        raise ValueError("DLC read outside ISO member")
                    return read(offset + pos, length)
                try:
                    platform = _platform(member_read, extent)
                except (ValueError, struct.error):
                    continue
                if platform:
                    found.append(DlcSource(path, platform, extent, member, offset))
            return found
    except (OSError, ValueError, UnicodeError, struct.error):
        return []


def detect_dlc(path):
    """Return hero LVL sources under ``path``, or an empty list when absent.

    All file and member names may be changed. Results are deterministic and
    include both PC and PS2 alternatives when both are present. The caller
    chooses one source; this function does not bake or copy anything.
    """
    path = Path(path).absolute()
    if path.is_file():
        return _detect_file(path)
    found = []
    if path.is_dir():
        for base, dirs, files in os.walk(path):
            dirs.sort()
            for name in sorted(files):
                target = Path(base) / name
                if target.is_file():  # opening a pipe would wait for a writer that never comes
                    found.extend(_detect_file(target))
    return found


def _fnv1a_lowercase(name):
    value = 2166136261
    for byte in name.encode():
        value = ((value ^ (byte | 0x20)) * 16777619) & 0xffffffff
    return value


def detect_hero_sounds(path):
    """The smallest LVL under a folder that defines Fisto's bubble sounds, or None. Known by the sound's name hash: the
    side LVL names the sound in plain text and larger map banks may repeat it, so the smallest is the heroes' own bank.
    Nothing else about the file, or its name, matters."""
    wanted = struct.pack("<I", _fnv1a_lowercase("hero_weap_energyBall_throw"))
    path, best = Path(path).absolute(), None
    if not path.is_dir():
        return None
    for base, dirs, files in os.walk(path):
        dirs.sort()
        for name in sorted(files):
            target = Path(base) / name
            try:
                if not target.is_file() or target.stat().st_size > MAX_SOUND_LVL:
                    continue
                data = target.read_bytes()
            except OSError:
                continue
            if data[:4] == b"ucfb" and wanted in data and (best is None or len(data) < best[0]):
                best = (len(data), target)
    return best and best[1]
