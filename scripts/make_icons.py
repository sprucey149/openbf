#!/usr/bin/env python3
"""make_icons.py -- the application icon for every platform, from one master.

    python3 scripts/make_icons.py                 write every asset
    python3 scripts/make_icons.py --check         fail when a committed asset does not
                                                  draw what the master draws
    python3 scripts/make_icons.py --master <png>  derive from another master file

branding/openbf.png is the single source: project artwork, not game data, so it is
tracked, and everything else in branding/ is derived from it (small, regenerable):

  branding/windows/openbf.ico                16,24,32,48,64,128,256 px, PNG-compressed
                                             entries. Windows reads the icon out of the
                                             .exe, so engine/crates/game/build.rs compiles
                                             this into game.exe.
  branding/macos/openbf.iconset/             iconutil -c icns input, in Apple's own names
  branding/macos/openbf.icns                 the same container, written as bytes here
  branding/linux/hicolor/<size>/apps/openbf.png   the icon-theme sizes
  branding/linux/openbf.desktop.template     @EXE@ is the path of the packed game

Resampling is Lanczos. The master is 750 px, so the 1024 px entry (icon_512x512@2x,
ic10) is an upscale: replace the master with a 1024 px drawing when the art is
redrawn. The .icns is written as bytes because macOS 10.7+ reads PNG in every entry
type, so a Windows or Linux box produces it. On macOS, `iconutil -c icns
branding/macos/openbf.iconset -o branding/macos/openbf.icns` is the Apple-native path
from the committed .iconset and stays authoritative if the format changes. --check
compares what each asset draws (pixels and the set of entries), not bytes, so a
different Pillow version is not reported as stale.
Exit: 0 ok, 1 a committed asset is stale, 2 a bad argument or a missing master.
"""

import argparse
import hashlib
import io
import struct
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
MASTER = ROOT / "branding" / "openbf.png"

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

DESKTOP = """[Desktop Entry]
Type=Application
Name=OpenBF
Comment=Star Wars Battlefront 1 and 2, built from copies you own
Exec=@EXE@
Icon=openbf
Terminal=false
Categories=Game;StrategyGame;
StartupWMClass=openbf
"""


def load_master(path):
    return Image.open(path).convert("RGBA")


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
    """Every asset, path -> bytes, derived from the master image."""
    files = {"branding/windows/openbf.ico": ico_bytes(img)}
    for name, px in ICONSET:
        files["branding/macos/openbf.iconset/" + name] = png_bytes(scale(img, px))
    files["branding/macos/openbf.icns"] = icns_bytes(img)
    for px in HICOLOR_SIZES:
        files[f"branding/linux/hicolor/{px}x{px}/apps/openbf.png"] = png_bytes(scale(img, px))
    files["branding/linux/openbf.desktop.template"] = DESKTOP.encode("utf-8")
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
        else:
            out[rel] = [("text", hashlib.sha256(payload).hexdigest()[:16])]
    return out


def write(files, tree=ROOT):
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


def check(files, tree=ROOT):
    """The committed assets that do not draw what the master draws, as notes."""
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
                    help="report committed assets that do not match the master, and write nothing")
    ap.add_argument("--master", default=str(MASTER),
                    help="the PNG to derive from (default: branding/openbf.png)")
    a = ap.parse_args()
    master = Path(a.master)
    if not master.is_file():
        sys.exit(f"make_icons: master {master} not found")
    files = assets(load_master(master))
    if a.check:
        stale = check(files)
        for note in stale:
            print(f"  {note}")
        if stale:
            sys.exit(f"make_icons: {len(stale)} of {len(files)} assets are not what {master} gives "
                     f"(run python3 scripts/make_icons.py)")
        print(f"make_icons: {len(files)} assets match {master}")
        return
    written = write(files)
    print(f"make_icons: wrote {len(written)} of {len(files)} asset(s) under {ROOT / 'branding'}")


if __name__ == "__main__":
    main()
