#!/usr/bin/env python3
"""make_icons.py -- the application icon for every platform, from one master PNG.

    python3 scripts/make_icons.py                 write every container
    python3 scripts/make_icons.py --check         fail when a container does not draw
                                                  what the master draws
    python3 scripts/make_icons.py --master <png> --out <dir>

No icon artwork is kept in the repository. scripts/pack.py renders the master from the
player's own bake (`game --render-icon`: the clone trooper's helmet, docs/branding.md)
into engine/branding_baked/openbf.png, which git ignores, and runs this script, which
writes beside it:

  windows/openbf.ico                         16,24,32,48,64,128,256 px, PNG-compressed
                                             entries. Windows reads the icon out of the
                                             .exe, so engine/crates/game/build_icon.rs
                                             compiles this into game.exe when pack.py
                                             names it in OPENBF_ICON.
  macos/openbf.iconset/                      iconutil -c icns input, in Apple's own names
  macos/openbf.icns                          the same container, written as bytes here
  linux/hicolor/<size>/apps/openbf.png       the icon-theme sizes scripts/install_linux.sh
                                             installs

Resampling is Lanczos; the rendered master is 1024 px, so every entry is a downscale.
The .icns is written as bytes because macOS 10.7+ reads PNG in every entry type, so a
Windows or Linux box produces it. --check compares what each container draws (pixels and
the set of entries), not bytes, so a different Pillow version is not reported as stale.
Exit: 0 ok, 1 a container is stale, 2 a bad argument or a missing master.
"""

import argparse
import hashlib
import io
import struct
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
# Generated and git-ignored: scripts/pack.py owns this folder.
OUT = ROOT / "engine" / "branding_baked"
MASTER = OUT / "openbf.png"

# The sizes Windows asks for: 16 at 100 % zoom, 24 at 150 %, 32 at 200 %, 48 at
# 300 %, then 64, 128 and 256 for Explorer's large views.
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)
# The freedesktop icon-theme sizes, named `<N>x<N>/apps/openbf.png`.
HICOLOR_SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
# iconutil's own names; an @2x entry doubles the logical size.
ICONSET = [("icon_16x16.png", 16), ("icon_16x16@2x.png", 32), ("icon_32x32.png", 32),
           ("icon_32x32@2x.png", 64), ("icon_128x128.png", 128), ("icon_128x128@2x.png", 256),
           ("icon_256x256.png", 256), ("icon_256x256@2x.png", 512),
           ("icon_512x512.png", 512), ("icon_512x512@2x.png", 1024)]
# Four-character entry types: icp4..ic10 are the 1x sizes, ic11..ic14 the Retina doubles.
ICNS_TYPES = [("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256),
             ("ic09", 512), ("ic10", 1024), ("ic11", 32), ("ic12", 64), ("ic13", 256),
             ("ic14", 512)]

def load_master(path):
    """The master as RGBA, padded to a square with transparency when it is not one."""
    img = Image.open(path).convert("RGBA")
    if img.width != img.height:
        edge = max(img.size)
        square = Image.new("RGBA", (edge, edge), (0, 0, 0, 0))
        square.paste(img, ((edge - img.width) // 2, (edge - img.height) // 2))
        img = square
    return img


def scale(img, size):
    if img.size == (size, size):
        return img
    return img.resize((size, size), Image.Resampling.LANCZOS)


def png_bytes(img):
    out = io.BytesIO()
    img.save(out, "PNG", optimize=True, compress_level=9)
    return out.getvalue()


def ico_bytes(img):
    """The .ico container: the directory, then each entry's PNG. 256 px writes as 0."""
    entries = [(px, png_bytes(scale(img, px))) for px in ICO_SIZES]
    offset = 6 + 16 * len(entries)
    directory = b""
    for px, payload in entries:
        directory += struct.pack("<BBBBHHII", px % 256, px % 256, 0, 0, 1, 32, len(payload), offset)
        offset += len(payload)
    head = b"\x00\x00\x01\x00" + struct.pack("<H", len(entries))
    return head + directory + b"".join(payload for _, payload in entries)


def icns_bytes(img):
    """The .icns container: magic, the whole length, then type + length + PNG per entry.
    A length field counts its own 8-byte header (the type and the length), so an entry
    of 8 + payload bytes is read as that type, then its payload, then the next entry.
    """
    body = b""
    for key, px in ICNS_TYPES:
        payload = png_bytes(scale(img, px))
        body += key.encode("ascii") + struct.pack(">I", 8 + len(payload)) + payload
    return b"icns" + struct.pack(">I", 8 + len(body)) + body


def ico_entries(data):
    """Read the .ico back: [(px, payload)], the 256 px entry stored as the byte 0."""
    if data[:4] != b"\x00\x00\x01\x00":
        raise ValueError("not an .ico: the header is not reserved + type 1")
    count = struct.unpack("<H", data[4:6])[0]
    out = []
    for i in range(count):
        w, h, _colors, _reserved, _planes, _bits, size, offset = struct.unpack(
            "<BBBBHHII", data[6 + 16 * i:22 + 16 * i])
        px = 256 if (w == 0 and h == 0) else (w or 256)
        out.append((px, data[offset:offset + size]))
    return out


def icns_entries(data):
    """Read the .icns back: {type: payload}. A declared length that is not the file is an error."""
    if data[:4] != b"icns":
        raise ValueError("not an .icns: the magic is missing")
    total = struct.unpack(">I", data[4:8])[0]
    if total != len(data):
        raise ValueError(f"not an .icns: it declares {total} bytes, the file is {len(data)}")
    out, at = {}, 8
    while at < total:
        key = data[at:at + 4].decode("ascii")
        size = struct.unpack(">I", data[at + 4:at + 8])[0]
        out[key] = data[at + 8:at + size]
        at += size
    return out


def assets(img):
    """Every container, path under the output folder -> bytes, derived from the master."""
    files = {"windows/openbf.ico": ico_bytes(img)}
    for name, px in ICONSET:
        files["macos/openbf.iconset/" + name] = png_bytes(scale(img, px))
    files["macos/openbf.icns"] = icns_bytes(img)
    for px in HICOLOR_SIZES:
        files[f"linux/hicolor/{px}x{px}/apps/openbf.png"] = png_bytes(scale(img, px))
    return files


def fingerprint(payload):
    """What a PNG draws: its size and a hash of its pixels, so a re-encode is not stale."""
    img = Image.open(io.BytesIO(payload))
    return (img.size, hashlib.sha256(img.tobytes()).hexdigest()[:16])


def fingerprints(files):
    """Each asset as the entries it draws, keyed by its path."""
    out = {}
    for rel, payload in files.items():
        if rel.endswith(".ico"):
            out[rel] = sorted((str(px), fingerprint(body)) for px, body in ico_entries(payload))
        elif rel.endswith(".icns"):
            out[rel] = sorted((key, fingerprint(body)) for key, body in icns_entries(payload).items())
        elif rel.endswith(".png"):
            out[rel] = [("png", fingerprint(payload))]
    return out


def write(files, tree=OUT):
    """Write every asset that is not already exactly right; return the paths written."""
    written = []
    for rel, payload in files.items():
        path = tree / rel
        if path.is_file() and path.read_bytes() == payload:
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        written.append(rel)
    return written


def check(files, tree=OUT):
    """The containers that do not draw what the master draws, as notes."""
    want = fingerprints(files)
    stale = []
    for rel, entries in want.items():
        path = tree / rel
        if not path.is_file():
            stale.append(f"{rel}: missing")
            continue
        try:
            got = fingerprints({rel: path.read_bytes()})[rel]
        except ValueError as error:
            stale.append(f"{rel}: {error}")
            continue
        if got != entries:
            stale.append(f"{rel}: {len(got)} entries drawn, {len(entries)} from the master differ")
    return stale


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true",
                    help="report containers that do not match the master, and write nothing")
    ap.add_argument("--master", default=None,
                    help="the PNG to derive from (default: <out>/openbf.png, which pack.py renders)")
    ap.add_argument("--out", default=str(OUT),
                    help="the folder the containers go to (default: engine/branding_baked)")
    a = ap.parse_args()
    out = Path(a.out)
    master = Path(a.master) if a.master else out / "openbf.png"
    if not master.is_file():
        print(f"make_icons: master {master} not found (scripts/pack.py renders it from your bake)",
              file=sys.stderr)
        sys.exit(2)
    files = assets(load_master(master))
    if a.check:
        stale = check(files, out)
        for note in stale:
            print(f"  {note}")
        if stale:
            sys.exit(f"make_icons: {len(stale)} of {len(files)} containers are not what {master} gives "
                     f"(run python3 scripts/make_icons.py)")
        print(f"make_icons: {len(files)} containers match {master}")
        return
    written = write(files, out)
    print(f"make_icons: wrote {len(written)} of {len(files)} container(s) under {out}")


if __name__ == "__main__":
    main()
