#!/usr/bin/env python3
"""A4: call-graph reachability for SWBF PS2 decomp corpora.

Streams a Ghidra decomp C export (functions delimited by
'/* ==== <addr> <name> ==== */' markers), extracts direct call edges
(known function identifier followed by '(') and address-taken references
(known function identifier NOT followed by '('), then runs reachability
BFS from a set of roots (Lua bindings + startup anchor).

Usage:
  python3 scripts/callgraph.py --game bf1 [--roots extra_roots.txt]
  python3 scripts/callgraph.py --game bf2 --outdir analysis
Roots file format: one root per line, either an 8-hex address or a name
('#' comments allowed). These are ADDED to the default roots
(bindings file + entry + startup anchor).
"""
import argparse
import re
from collections import defaultdict, deque
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

GAMES = {
    "bf1": {
        "decomp": "analysis/SWBFRONT.ELF-decomp.c",
        "census": "analysis/SWBFRONT.ELF-functions.txt",
        "bindings": "analysis/bf1-bindings.txt",
        "stringrefs": "analysis/SWBFRONT.ELF-stringrefs.txt",
        "elf": "extracted/bf1/SWBFRONT.ELF",
        # entry -> FUN_002e8950 (startup); entry itself is the anchor root
        "anchors": ["00160008", "002e8950"],
    },
    "bf2": {
        "decomp": "analysis/SWBFRNT2.ELF-decomp.c",
        "census": "analysis/SWBFRNT2.ELF-functions.txt",
        "bindings": "analysis/bf2-bindings.txt",
        "stringrefs": "analysis/SWBFRNT2.ELF-stringrefs.txt",
        "elf": "extracted/bf2/SWBFRNT2.ELF",
        # entry, startup FUN_003c1c38
        "anchors": ["00160008", "003c1c38"],
    },
}

# Dropped-region string patterns (case-insensitive except where noted).
DROP_PATTERNS = [
    ("gamespy", re.compile(r"gamespy|gp(cm|sp)\.|natneg|qr2|peerchat", re.I)),
    ("dnas", re.compile(r"dnas", re.I)),
    ("eenet", re.compile(r"eenet|ent_|libnet", re.I)),
    ("ppp", re.compile(r"\bppp|pppoe", re.I)),
    ("headset/gsm", re.compile(r"headset|\bgsm\b|logitech", re.I)),
    ("memcard", re.compile(r"memcard|memory ?card|mc[01]:|sceMc|McSemaphore", re.I)),
    ("usb", re.compile(r"\busb|lgaud|sceUsbd", re.I)),
]

HEADER_RE = re.compile(r"^/\* ==== ([0-9a-f]{8}) (\S+) ==== \*/")
IDENT_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def load_census(path):
    """addr -> (size_bytes, name)"""
    census = {}
    with open(path) as f:
        for line in f:
            parts = line.split()
            if len(parts) == 3:
                addr, size, name = parts
                census[addr] = (int(size), name)
    return census


def load_bindings(path):
    """name -> addr and addr -> name for renamed (Lua-binding) functions."""
    n2a, a2n = {}, {}
    with open(path) as f:
        for line in f:
            parts = line.split()
            if len(parts) == 2:
                addr, name = parts
                n2a[name] = addr
                a2n[addr] = name
    return n2a, a2n


HEXLIT_RE = re.compile(r"0x([0-9a-f]{6,8})\b")


def parse_decomp(decomp_path, name2addr, census):
    """Stream the decomp file (two passes). Returns:
       funcs: addr -> line_count
       calls: addr -> set(addr)   direct calls (identifier followed by '(')
       takes: addr -> set(addr)   address-taken (identifier NOT followed by '(')
       lits:  addr -> set(addr)   hex literal equal to a census function start
                                  (function pointers passed as constants, e.g.
                                  thread entry points: FUN_x(...,0x2e8b70,...))
    Pass 1 collects the definition headers so every defined name resolves
    (FUN_*, thunk_FUN_*, entry, renamed bindings). Tokenization is
    whole-identifier, so PTR_ScriptCB_Foo_005f914c does NOT match
    ScriptCB_Foo. Thunks get an implicit call edge to their target.
    """
    funcs = {}
    calls = defaultdict(set)
    takes = defaultdict(set)
    lits = defaultdict(set)

    # pass 1: headers -> name resolution for everything defined in this corpus
    with open(decomp_path, errors="replace") as f:
        for line in f:
            m = HEADER_RE.match(line)
            if m:
                addr, nm = m.group(1), m.group(2)
                name2addr.setdefault(nm, addr)
                tm = re.fullmatch(r"thunk_FUN_([0-9a-f]{8})", nm)
                if tm:
                    calls[addr].add(tm.group(1))  # thunk -> target

    def resolve(tok):
        a = name2addr.get(tok)
        if a is not None:
            return a
        if tok.startswith("FUN_") and len(tok) == 12:
            return tok[4:]
        return None

    cur = None          # addr of current function
    cur_lines = 0
    with open(decomp_path, errors="replace") as f:
        for line in f:
            m = HEADER_RE.match(line)
            if m:
                if cur is not None:
                    funcs[cur] = funcs.get(cur, 0) + cur_lines
                cur = m.group(1)
                cur_lines = 0
                continue
            if cur is None:
                continue
            cur_lines += 1
            for m2 in IDENT_RE.finditer(line):
                tok = m2.group(0)
                tgt = resolve(tok)
                if tgt is None or tgt == cur:
                    continue
                # is the next non-space char a '(' ?
                rest = line[m2.end():m2.end() + 2].lstrip()
                if rest.startswith("("):
                    calls[cur].add(tgt)
                else:
                    takes[cur].add(tgt)
            if "0x" in line:
                for m3 in HEXLIT_RE.finditer(line):
                    a = m3.group(1).zfill(8)
                    if a in census and a != cur:
                        lits[cur].add(a)
        if cur is not None:
            funcs[cur] = funcs.get(cur, 0) + cur_lines
    return funcs, calls, takes, lits


def bfs(roots, *edge_maps):
    seen = set()
    q = deque(r for r in roots)
    seen.update(q)
    while q:
        n = q.popleft()
        for em in edge_maps:
            for t in em.get(n, ()):
                if t not in seen:
                    seen.add(t)
                    q.append(t)
    return seen


def scan_elf_data_pointers(elf_path, census_addrs):
    """Scan non-executable PROGBITS sections (.data, .rodata, ...) of a
    32-bit LE ELF for 4-byte words whose value equals a census function
    start address. These are vtable slots / callback tables / function
    pointers in initialized data — the call targets that indirect calls
    (through a function pointer in the decomp) reach but that never appear as a
    textual FUN_ reference. Returns set of function addrs found in data.
    """
    import struct
    found = set()
    with open(elf_path, "rb") as f:
        eh = f.read(52)
        if eh[:4] != b"\x7fELF" or eh[4] != 1 or eh[5] != 1:
            raise ValueError("expected 32-bit little-endian ELF")
        shoff, = struct.unpack_from("<I", eh, 32)
        shentsize, shnum = struct.unpack_from("<HH", eh, 46)
        f.seek(shoff)
        sh = f.read(shentsize * shnum)
        for i in range(shnum):
            (nameoff, typ, flags, addr, off, size, link, info, align,
             entsz) = struct.unpack_from("<10I", sh, i * shentsize)
            SHT_PROGBITS, SHF_ALLOC, SHF_EXEC = 1, 0x2, 0x4
            if typ != SHT_PROGBITS or not (flags & SHF_ALLOC) or (flags & SHF_EXEC):
                continue
            f.seek(off)
            data = f.read(size)
            for j in range(0, len(data) - 3, 4):
                v = int.from_bytes(data[j:j + 4], "little")
                a = format(v, "08x")
                if a in census_addrs:
                    found.add(a)
    return found


def load_stringref_tags(path):
    """addr -> set(tag) for functions referencing dropped-region strings."""
    tags = defaultdict(set)
    fun_re = re.compile(r"FUN_([0-9a-f]{8})@")
    with open(path, errors="replace") as f:
        for line in f:
            parts = line.split("\t")
            if len(parts) < 3:
                continue
            s = parts[1]
            hit = [name for name, pat in DROP_PATTERNS if pat.search(s)]
            if not hit:
                continue
            for m in fun_re.finditer(parts[2]):
                for h in hit:
                    tags[m.group(1)].add(h)
    return tags


def cluster_bands(tags, census, gap=0x8000, min_seeds=4):
    """Cluster dropped-seed functions into contiguous address bands
    (statically linked libraries are contiguous in the binary). Returns
    (bands, band_addrs): bands as (lo, hi, n_seeds, tagset, fns, bytes),
    and the set of census fn addrs inside bands with >= min_seeds seeds
    (the band-based dropped estimate).
    """
    addrs = sorted(tags)
    bands = []
    for a in addrs:
        v = int(a, 16)
        if bands and v - bands[-1][1] < gap:
            bands[-1][1] = v
            bands[-1][2] += 1
            bands[-1][3].update(tags[a])
        else:
            bands.append([v, v, 1, set(tags[a])])
    out, band_addrs = [], set()
    fns = [(int(x, 16), x, s) for x, (s, _) in census.items()]
    for lo, hi, n, ts in bands:
        inband = [(x, a, s) for x, a, s in fns if lo <= x <= hi]
        out.append((lo, hi, n, ts, len(inband), sum(s for _, _, s in inband)))
        if n >= min_seeds:
            band_addrs.update(a for _, a, _ in inband)
    return out, band_addrs


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--game", choices=GAMES, required=True)
    ap.add_argument("--roots", help="extra roots file (addr or name per line)")
    ap.add_argument("--outdir", default=None,
                    help="dir for edge-list dumps (default: no dumps)")
    ap.add_argument("--top", type=int, default=30, help="hub table size")
    args = ap.parse_args()
    cfg = GAMES[args.game]

    census = load_census(REPO / cfg["census"])
    name2addr, addr2name = load_bindings(REPO / cfg["bindings"])
    name2addr["entry"] = "00160008"
    addr2name["00160008"] = "entry"

    funcs, calls, takes, lits = parse_decomp(REPO / cfg["decomp"], name2addr, census)

    roots = set(a for a in addr2name if a in funcs or a in census)
    roots.update(a for a in cfg["anchors"] if a in funcs)
    if args.roots:
        with open(args.roots) as f:
            for line in f:
                line = line.split("#")[0].strip()
                if not line:
                    continue
                a = name2addr.get(line, line if re.fullmatch(r"[0-9a-f]{8}", line) else None)
                if a:
                    roots.add(a)

    n_edges = sum(len(v) for v in calls.values())
    n_takes = sum(len(v) for v in takes.values())
    n_lits = sum(len(v) for v in lits.values())
    strict = bfs(roots, calls)
    closure = bfs(roots, calls, takes)
    full = bfs(roots, calls, takes, lits)

    # conservative-linker tier: functions whose address sits in initialized
    # data (vtables, callback tables) are roots too — indirect-call targets
    datarefs = scan_elf_data_pointers(REPO / cfg["elf"], set(census))
    linker = bfs(roots | datarefs, calls, takes, lits)

    tags = load_stringref_tags(REPO / cfg["stringrefs"])
    seeds = set(tags)
    bands, band_addrs = cluster_bands(tags, census)
    dropped = seeds | band_addrs   # seed fns + whole library bands
    reach_drop = linker & dropped
    keep = linker - dropped

    # carve-off BFS: reachability when dropped functions are removed
    def prune(em):
        return {k: {t for t in v if t not in dropped}
                for k, v in em.items() if k not in dropped}
    core = bfs((roots | datarefs) - dropped, prune(calls), prune(takes),
               prune(lits))

    def size_of(addrs):
        return sum(census[a][0] for a in addrs if a in census)

    total_bytes = sum(s for s, _ in census.values())

    def name(a):
        return addr2name.get(a, census.get(a, (0, "FUN_" + a))[1])

    fan_in = defaultdict(int)
    for s, ts in calls.items():
        for t in ts:
            fan_in[t] += 1
    fan_out = {k: len(v) for k, v in calls.items()}

    print(f"== {args.game}: {cfg['decomp']} ==")
    print(f"functions parsed (decomp): {len(funcs)}   census: {len(census)}")
    print(f"call edges: {n_edges}   address-taken refs: {n_takes}   "
          f"literal-addr refs: {n_lits}")
    print(f"roots: {len(roots)} (bindings + entry + startup anchor)")
    print(f"reachable strict (calls only): {len(strict)} fns, "
          f"{size_of(strict)} bytes ({100*size_of(strict)/total_bytes:.1f}%)")
    print(f"reachable closure (+addr-taken): {len(closure)} fns, "
          f"{size_of(closure)} bytes ({100*size_of(closure)/total_bytes:.1f}%)")
    print(f"reachable full (+literal fn addrs): {len(full)} fns, "
          f"{size_of(full)} bytes ({100*size_of(full)/total_bytes:.1f}%)")
    print(f"data-resident fn pointers (ELF .data/.rodata): {len(datarefs)}")
    print(f"reachable linker tier (+data-ptr roots): {len(linker)} fns, "
          f"{size_of(linker)} bytes ({100*size_of(linker)/total_bytes:.1f}%)")
    print(f"dropped: {len(seeds)} string-tagged seeds -> "
          f"{len(dropped)} fns incl. library bands "
          f"({size_of(dropped)} bytes)   reachable&dropped: {len(reach_drop)}")
    print("dropped-region bands (>=4 seeds are carved):")
    for lo, hi, n, ts, nf, nb in bands:
        mark = "*" if n >= 4 else " "
        print(f"  {mark} {lo:08x}-{hi:08x} seeds={n:3d} fns={nf:5d} "
              f"bytes={nb:7d} {','.join(sorted(ts))}")
    print(f"HEADLINE keep-set (linker - dropped): {len(keep)} fns, "
          f"{size_of(keep)} bytes ({100*size_of(keep)/total_bytes:.1f}%)")
    print(f"core after carving dropped fns out of graph: {len(core)} fns, "
          f"{size_of(core)} bytes ({100*size_of(core)/total_bytes:.1f}%)")

    def hub_table(counter, label):
        print(f"\ntop {args.top} by {label}:")
        rows = sorted(counter.items(), key=lambda kv: -kv[1])[: args.top]
        for a, c in rows:
            sz = census.get(a, (0, ""))[0]
            ln = funcs.get(a, 0)
            r = "R" if a in linker else "-"
            d = "D" if a in dropped else "-"
            print(f"  {a} {name(a):<32} {label}={c:<5} {sz:>6}B {ln:>5}ln {r}{d}")

    hub_table(fan_in, "fan-in")
    hub_table(fan_out, "fan-out")

    if args.outdir:
        out = Path(args.outdir)
        out.mkdir(parents=True, exist_ok=True)
        with open(out / f"a4-{args.game}-edges.txt", "w") as f:
            for s in sorted(calls):
                for t in sorted(calls[s]):
                    f.write(f"{s} C {t}\n")
            for s in sorted(takes):
                for t in sorted(takes[s]):
                    f.write(f"{s} T {t}\n")
            for s in sorted(lits):
                for t in sorted(lits[s]):
                    f.write(f"{s} L {t}\n")
        with open(out / f"a4-{args.game}-reachable.txt", "w") as f:
            for a in sorted(linker):
                tier = ("strict" if a in strict
                        else "closure" if a in closure
                        else "literal" if a in full else "dataptr")
                flags = ("drop:" + ",".join(sorted(tags[a]))) if a in tags else ""
                f.write(f"{a} {name(a)} {tier} {flags}\n")
        print(f"\nwrote {out}/a4-{args.game}-edges.txt and -reachable.txt")


if __name__ == "__main__":
    main()
