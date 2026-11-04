#!/usr/bin/env python3
"""Recognize supported console BF2 input before a PC asset bake.

PS2 supplies the shell/HUD and gameplay reference. These are the content readers;
scripts/input_scan.py decides what a folder of inputs adds up to. An unrecognized
image never satisfies a requirement.
"""
from pathlib import Path
import argparse
import struct

REQUIRED = {"DATA/_LVL_PS2/CORE.LVL", "DATA/_LVL_PS2/INGAME.LVL", "DATA/_LVL_PS2/SHELL.LVL", "SWBFRNT2.ELF", "SYSTEM.CNF"}
REQUIRED_BF1 = {"DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL", "SWBFRONT.ELF", "SYSTEM.CNF"}
MISSING = "PC input requires a console Battlefront II disc: add a PS2 BF2 disc with --iso2 (or extract it under extracted/bf2/lvl). Xbox BF2 support is not available yet."


def ci_path(base, rel):
    cur = Path(base)
    for part in rel.replace("\\", "/").split("/"):
        direct = cur / part
        if direct.exists():
            cur = direct
        elif cur.is_dir():
            cur = next((p for p in cur.iterdir() if p.name.lower() == part.lower()), direct)
        else:
            return direct
    return cur


def chunks(data):
    off = 0
    while off < len(data):
        if len(data) - off < 8:
            raise ValueError("truncated LVL chunk")
        size = struct.unpack_from("<I", data, off + 4)[0]
        end = off + 8 + size
        if end > len(data):
            raise ValueError("truncated LVL payload")
        yield bytes(data[off:off + 4]), data[off + 8:end]
        off = (end + 3) & ~3


def geometry_platform(path):
    """"ps2", "pc", "xbox" or None, from an LVL's structure.

    Geometry splits PS2 (`_PS2`) from the rest (`VBUF`/`VDAT`). The Xbox shares that
    geometry, so a texture decides between the two: PC `tex_` holds `FMT_` chunks,
    Xbox `tex_` a bare `BODY`, PS2 `tex_` a `pal_` and a `BODY` (measured on all three).
    """
    try:
        path = Path(path)
        if path.stat().st_size > 128 << 20:
            return None
        blob = memoryview(path.read_bytes())
        if len(blob) < 8 or blob[:4] != b"ucfb" or struct.unpack_from("<I", blob, 4)[0] != len(blob) - 8:
            return None
        pending = [(b"ucfb", blob[8:])]
        count = 0
        vertex_buffers, xbox_texture, pc_texture = False, False, False
        # Only inspect structural chunks; matching text in an image is not evidence.
        while pending:
            parent, data = pending.pop()
            for tag, body in chunks(data):
                count += 1
                if count > 65536:
                    pending = []
                    break
                if parent == b"segm" and tag == b"_PS2":
                    return "ps2"
                if parent == b"segm" and tag in (b"VBUF", b"VDAT"):
                    vertex_buffers = True
                if tag == b"tex_" and not (xbox_texture or pc_texture):
                    inner = {child for child, _ in chunks(body)}
                    pc_texture = b"FMT_" in inner
                    xbox_texture = b"BODY" in inner and b"pal_" not in inner and not pc_texture
                if vertex_buffers and (pc_texture or xbox_texture):
                    return "xbox" if xbox_texture else "pc"
                if tag == b"lvl_" and len(body) >= 8:
                    pending.append((tag, body[8:]))
                elif tag in (b"ucfb", b"modl", b"segm"):
                    pending.append((tag, body))
        return "pc" if vertex_buffers else None
    except (OSError, ValueError, struct.error):
        pass
    return None


def is_ps2_lvl(path):
    return geometry_platform(path) == "ps2"


def pc_game(root):
    if not ci_path(root, "CORE.LVL").is_file():
        return None
    for game, name in (("bf2", "INGAME.LVL"), ("bf1", "COMMON/ENG/COMMON.LVL")):
        if geometry_platform(ci_path(root, name)) == "pc":
            return game
    return None


def is_ps2_bf2_tree(root):
    return all(ci_path(root, name).is_file() for name in ("CORE.LVL", "INGAME.LVL", "SHELL.LVL")) and is_ps2_lvl(ci_path(root, "INGAME.LVL"))


def ps2_disc_files(path):
    """The files of a PS2 disc (an ISO9660 image whose system ID says PLAYSTATION), else nothing."""
    system, files = iso9660_files(path)
    return files if system == b"PLAYSTATION" else set()


def iso9660_files(path):
    """(system ID, files): bounded ISO9660 directory inspection, independent of the image filename."""
    try:
        with Path(path).open("rb") as source:
            length = source.seek(0, 2)
            def read(offset, size):
                if offset < 0 or size < 0 or offset + size > length:
                    raise ValueError("ISO extent outside the image")
                source.seek(offset)
                data = source.read(size)
                if len(data) != size:
                    raise ValueError("truncated ISO extent")
                return data
            pvd = read(16 * 2048, 2048)
            if pvd[:7] != b"\x01CD001\x01":
                return None, set()
            system = pvd[8:40].rstrip(b" \0")
            stack = [("", struct.unpack_from("<I", pvd, 158)[0], struct.unpack_from("<I", pvd, 166)[0])]
            seen, files = set(), set()
            records, total = 0, 0
            while stack:
                prefix, lba, size = stack.pop()
                if lba in seen:
                    continue
                seen.add(lba)
                total += size
                if len(seen) > 4096 or size > 4 << 20 or total > 16 << 20 or len(prefix) > 4096:
                    return system, set()
                data = read(lba * 2048, size)
                off = 0
                while off < len(data):
                    size = data[off]
                    if not size:
                        off = (off // 2048 + 1) * 2048
                        continue
                    if size < 34 or off + size > len(data):
                        return system, set()
                    records += 1
                    if records > 65536:
                        return system, set()
                    rec = data[off:off + size]
                    off += size
                    n = rec[32]
                    if n + 33 > len(rec):
                        return system, set()
                    name = rec[33:33+n]
                    if name in (b"\0", b"\1"):
                        continue
                    name = name.decode("ascii").split(";", 1)[0].upper()
                    full = prefix + name
                    child_lba, child_size = struct.unpack_from("<I", rec, 2)[0], struct.unpack_from("<I", rec, 10)[0]
                    if rec[25] & 2:
                        stack.append((full + "/", child_lba, child_size))
                    else:
                        if child_lba * 2048 + child_size > length:
                            return system, set()
                        files.add(full)
            return system, files
    except (OSError, ValueError, UnicodeError, struct.error):
        return None, set()


def ps2_bf2_disc(path):
    return REQUIRED <= ps2_disc_files(path)


def require_console_bf2(pc_present, root, image=None):
    if pc_present and not is_ps2_bf2_tree(root) and not (image and ps2_bf2_disc(image)):
        raise ValueError(MISSING)


def require_ps2_bf1_reference(pc_present, root, image=None):
    if not pc_present:
        return
    tree = is_ps2_lvl(ci_path(root, "COMMON/ENG/COMMON.LVL"))
    disc = image and REQUIRED_BF1 <= ps2_disc_files(image)
    if not tree and not disc:
        raise ValueError("This PC BF1 build needs verified BF1 PS2 gameplay input for missions, classes and collision. "
                         "Add its disc with --iso1 or extract it under extracted/bf1/lvl.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pc", choices=("bf1", "bf2"))
    parser.add_argument("root")
    args = parser.parse_args()
    if args.pc:
        parser.exit(0 if pc_game(args.root) == args.pc else 2)
    if not is_ps2_bf2_tree(args.root):
        parser.exit(2, "rebake: " + MISSING + "\n")
