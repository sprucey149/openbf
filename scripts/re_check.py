#!/usr/bin/env python3
"""re_check.py — the RE self-checks as one-line tools (prompt §RE). Corpora: b1 = analysis/SWBFRONT.ELF-decomp.c,
b2 = analysis/SWBFRNT2.ELF-decomp.c; ELFs: extracted/bf1/SWBFRONT.ELF, extracted/bf2/SWBFRNT2.ELF (off = va - 0x160000 + 0x1000).

  re_check.py hash MaxSpeed                 candidate name references in both annotated corpora, with fn output
  re_check.py fn b2 708831 708404            check 1: the function each line lives in (+ the line itself)
  re_check.py absent b2 DAT_006615f9 0x5c8 [--card ID] [--src PATH]… [--elf]
                                             check 2: every hit of each term in BOTH decomp corpora, counts + enclosing functions.
                                             --src PATH (repeatable; file or dir, e.g. engine/assets_baked, extracted/) also greps
                                             the raw bytes (ASCII and 0x… terms as LE u32); --elf also scans both ELFs, including
                                             executable MIPS lui + ori/addiu builds within 12 instructions.
                                             Always writes a RECEIPT (engine/artifacts/loop/receipts/<card>_<id>.json) and prints
                                             `[receipt:<id>]` — paste that tag next to every "no class authors X / absent" claim;
                                             `re_check.py check-absence analysis/re-x.md` refuses an absence claim without one.
  re_check.py check-absence analysis/re-x.md [--warn] [--base REF]
                                             require existing receipt tags on absence claims outside code fences.
  re_check.py writers b2 DAT_006615f9         check 5: every WRITE of a global (`X = ...`, incl. initialisers) with its function
  re_check.py word b2 0x60c1fc [n] [--f32|--u32|--str]   check 4: raw ELF words at a VA, offset formula stated
  re_check.py callers b2 FUN_005755b0         who calls a function (decomp text), with the caller's name
Check 3 (the twin) is judgement: open both functions with `fn` and walk their callers with `callers`.
Paste the commands and their output into the RE doc — that is the evidence the verifier and the auditor re-run.
"""
import datetime, glob, hashlib, json, os, re, struct, subprocess, sys

VERSION = 2  # receipt format / tool version
MAX_SRC_FILES = 200000
ELF_VA_BASE = 0x160000
ELF_FILE_BASE = 0x1000

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
if not os.path.exists(os.path.join(ROOT, "analysis/SWBFRONT.ELF-decomp.c")):
    # A worktree: the corpora are gitignored, so only the main checkout (the git common dir's parent) has them.
    import subprocess
    _common = subprocess.run(["git", "-C", ROOT, "rev-parse", "--path-format=absolute", "--git-common-dir"],
                             capture_output=True, text=True).stdout.strip()
    if _common:
        ROOT = os.path.dirname(_common)
CORP = {"b1": "analysis/SWBFRONT.ELF-decomp.c", "b2": "analysis/SWBFRNT2.ELF-decomp.c"}
ELF = {"b1": "extracted/bf1/SWBFRONT.ELF", "b2": "extracted/bf2/SWBFRNT2.ELF"}
HDR = re.compile(r"^/\* ==== ([0-9a-f]{8}) (\S+) ==== \*/")
_cache = {}


def lines(c):
    if c not in _cache:
        _cache[c] = open(os.path.join(ROOT, CORP[c]), errors="replace").read().split("\n")
    return _cache[c]


def header_index(c):
    key = c + "_h"
    if key not in _cache:
        _cache[key] = [(i + 1, m.group(2), m.group(1)) for i, l in enumerate(lines(c)) if (m := HDR.match(l))]
    return _cache[key]


def enclosing(c, n):
    import bisect
    h = header_index(c)
    i = bisect.bisect_right([x[0] for x in h], n) - 1
    return h[i] if i >= 0 else (0, "?", "?")


def fn(c, nums):
    for n in map(int, nums):
        hl, name, addr = enclosing(c, n)
        print(f"{c.upper()}:{n} is in {name} (0x{addr}, header at line {hl}): {lines(c)[n - 1].strip()[:160]}")


def loop_dir():
    return os.environ.get("OPENBF_LOOP_DIR") or os.path.join(ROOT, "engine/artifacts/loop")


def _hash_value(t):
    return int(t, 16) if re.fullmatch(r"0x[0-9a-fA-F]{1,8}", t) else None


def _text_forms(t):
    u = _hash_value(t)
    if u is None:
        return [t]
    forms = [t, f"0x{u:x}", str(u)]
    if u >= 0x80000000:
        forms += [f"0xffffffff{u:08x}", f"-0x{0x100000000-u:x}", str(u-0x100000000)]
    return list(dict.fromkeys(forms))


def _needles(t, elf=False):
    # Raw-source words must never feed the corpus text search.
    nb = [t.encode("latin-1", "replace")]
    u = _hash_value(t)
    if u is not None:
        nb.append(struct.pack("<I", u))
    return nb


BUILD_WINDOW = 12  # instructions after a lui searched for its ori/addiu (delay slots, interleaved code)


def _writes(word):
    """Destination GPR of a MIPS/R5900 word, or None (conservative decode for the build scan)."""
    op = word >> 26
    if op in (0, 0x1c):  # SPECIAL / MMI: rd
        return (word >> 11) & 31
    if op == 3:  # jal
        return 31
    if op in (0x11, 0x12) and (word >> 21) & 31 in (0, 1, 2):  # mfc/dmfc/cfc
        return (word >> 16) & 31
    if 8 <= op <= 15 or op in (0x18, 0x19, 0x1a, 0x1b, 0x1e, 0x37) or 0x20 <= op <= 0x27:
        return (word >> 16) & 31
    return None


def _branch_target(word, va):
    """Target VA of a branch/jump word at `va`, or None (jr/jalr targets are unknown)."""
    op = word >> 26
    if op in (2, 3):
        return ((va + 4) & 0xf0000000) | ((word & 0x3ffffff) << 2)
    if op in (4, 5, 6, 7, 20, 21, 22, 23) or (op == 1 and (word >> 16) & 31 in (0, 1, 2, 3, 16, 17, 18, 19)):
        imm = word & 0xffff
        return va + 4 + ((imm - 0x10000 if imm >= 0x8000 else imm) << 2)
    return None


def _ends_path(word):
    op = word >> 26
    return op == 2 or (op == 0 and word & 0x3f == 8) or (op == 4 and not (word >> 16) & 0x3ff)


def _code_builds(data, wanted):
    """MIPS lui + ori/addiu candidates in executable ELF32 LE load segments: every ori/addiu reading the
    lui register within BUILD_WINDOW instructions on the fall-through path and, for a lui in a branch delay
    slot, at the branch target; a path stops when the register is rewritten or after an unconditional
    jump's delay slot. A bounded syntax scan, not control-flow/dataflow proof of absence.
    """
    if len(data) < 52 or data[:7] != b"\x7fELF\x01\x01\x01":
        raise OSError("code scan requires ELF32 little-endian version 1")
    if struct.unpack_from("<H", data, 18)[0] != 8:
        raise OSError("code scan requires MIPS ELF")
    phoff = struct.unpack_from("<I", data, 28)[0]
    size, count = struct.unpack_from("<HH", data, 42)
    if size < 32 or not count or phoff + size * count > len(data):
        raise OSError("incomplete ELF program headers")
    out = []
    for index in range(count):
        typ, off, va, _, length, _, flags, _ = struct.unpack_from("<8I", data, phoff + index * size)
        if typ != 1 or not flags & 1:
            continue
        if off + length > len(data) or va % 4 or off % 4:
            raise OSError("incomplete or unaligned executable ELF segment")
        end = off + length - length % 4
        for pos in range(off, end - 4, 4):
            hi = struct.unpack_from("<I", data, pos)[0]
            reg = (hi >> 16) & 31
            if hi >> 26 != 15 or (hi >> 21) & 31 or not reg:
                continue
            starts = [pos + 4]
            if pos > off:  # a lui in a branch delay slot also continues at the branch target
                prev = struct.unpack_from("<I", data, pos - 4)[0]
                target = None if prev >> 26 == 3 else _branch_target(prev, va + pos - off - 4)  # not into a callee
                if target is not None and va <= target < va + (end - off) and target != va + pos + 4 - off:
                    starts.append(off + target - va)
            seen = set()
            for start in starts:
                stop = min(end, start + 4 * BUILD_WINDOW)
                q = start
                while q < stop:
                    lo = struct.unpack_from("<I", data, q)[0]
                    op = lo >> 26
                    if op in (13, 9) and (lo >> 21) & 31 == reg and (lo >> 16) & 31 and q not in seen:
                        seen.add(q)
                        low = lo & 0xffff
                        value = (((hi & 0xffff) << 16) | low) if op == 13 else (
                            ((hi & 0xffff) << 16) + (low if low < 0x8000 else low - 0x10000)) & 0xffffffff
                        if value in wanted:
                            out.append(dict(value=f"0x{value:08x}", offset=pos, va=va+pos-off,
                                            form="lui/ori" if op == 13 else "lui/addiu",
                                            consumer_va=va+q-off, via="branch target" if start != pos + 4 else "fall-through"))
                    if _writes(lo) == reg:
                        break
                    if _ends_path(lo):
                        stop = min(stop, q + 8)
                    q += 4
    return out


def _scan_bytes(path, needles, hit_files, counts, details, is_elf=False):
    try:
        with open(path, "rb") as f:
            data = f.read()
    except OSError as exc:
        raise OSError(f"incomplete source scan: {path}: {exc}") from exc
    relative = os.path.relpath(path, ROOT)
    matched = set()
    for t, nb in needles:
        for index, needle in enumerate(nb):
            start = 0
            while (offset := data.find(needle, start)) >= 0:
                counts[t] = counts.get(t, 0) + 1
                matched.add(t)
                details.append(dict(term=t, file=relative, offset=offset,
                                    form="ascii" if index == 0 else "le_u32", aligned=offset % 4 == 0))
                start = offset + 1
    if is_elf and any(_hash_value(t) is not None for t, _ in needles):
        builds = _code_builds(data, {_hash_value(t) for t, _ in needles})
        for t, _ in needles:
            for build in builds:
                if _hash_value(t) == int(build["value"], 16):
                    counts[t] = counts.get(t, 0) + 1
                    matched.add(t)
                    details.append(dict(term=t, file=relative, **build))
    for t, _ in needles:
        if t in matched:
            hit_files.setdefault(t, []).append(relative)
    return len(data)


def search(terms, srcs=(), elf=False, quiet=False, pc=False):
    """The absence search; returns the receipt dict (not yet written). Decomp corpora b1+b2 always; `srcs` raw files/dirs and
    (elf) the two ELFs are grepped as bytes. Prints the usual per-term report unless quiet."""
    rec = dict(version=VERSION, tool="re_check.py absent", terms=list(terms), corpora=[], sources=[], total_hits=0)
    for t in terms:
        forms = _text_forms(t)
        pat = re.compile("|".join((r"(?<!\w)" if re.match(r"\w", f) else "") + re.escape(f) +
                                  (r"\b" if f and re.match(r"\w", f[-1]) else "") for f in forms),
                         re.IGNORECASE if _hash_value(t) is not None else 0)
        for c in ("b1", "b2"):
            hits = [i + 1 for i, l in enumerate(lines(c)) if pat.search(l)]
            fns = {}
            for n in hits:
                fns.setdefault(enclosing(c, n)[1], []).append(n)
            rec["corpora"].append(dict(term=t, corpus=c, path=CORP[c], lines=len(lines(c)), hits=len(hits), functions=len(fns), forms=forms))
            rec["total_hits"] += len(hits)
            if not quiet:
                print(f"text forms: {forms}")
                print(f"{t!r} in {c.upper()}: {len(hits)} hit(s) in {len(fns)} function(s)")
                for f, ns in list(fns.items())[:40]:
                    print(f"   {f}: lines {', '.join(map(str, ns[:12]))}{' …' if len(ns) > 12 else ''}")
    paths = [(p, False) for p in srcs] + ([(ELF[c], True) for c in ("b1", "b2")] if elf else [])
    for p, is_elf in paths:
        full = p if os.path.isabs(p) else os.path.join(ROOT, p)
        if not p or not os.path.exists(full):
            raise OSError(f"requested source unavailable: {p!r}")
        def walk_error(exc):
            raise OSError(f"incomplete source traversal: {full}: {exc}") from exc
        if os.path.isfile(full):
            files = [full]
        elif os.path.isdir(full):
            files = []
            for d, _, fs in os.walk(full, onerror=walk_error):
                files.extend(os.path.join(d, f) for f in fs)
                if len(files) > MAX_SRC_FILES:
                    raise OSError(f"incomplete source scan: {p}: exceeds file budget {MAX_SRC_FILES}")
        else:
            raise OSError(f"requested source is not a file or directory: {p}")
        needles = [(t, _needles(t, is_elf)) for t in terms]
        counts, hit_files, nbytes, details = {}, {}, 0, []
        for f in files:
            nbytes += _scan_bytes(f, needles, hit_files, counts, details, is_elf)
        hits = sum(counts.values())
        rec["sources"].append(dict(path=p, kind="elf" if is_elf else "raw", files=len(files), bytes=nbytes, hits=hits,
                                   per_term={t: counts.get(t, 0) for t in terms},
                                   needles={t: [{"form": "ascii" if i == 0 else "le_u32", "hex": n.hex()}
                                                for i, n in enumerate(nb)] for t, nb in needles},
                                   code_scan=f"lui then ori/addiu on its register within {BUILD_WINDOW} instructions (fall-through, and branch target for a delay-slot lui) in executable PT_LOAD; builds outside the window, through other registers or other instructions not covered" if is_elf else None,
                                   hit_files=hit_files, matches=details))
        rec["total_hits"] += hits
        if not quiet:
            print(f"raw {p}: {len(files)} file(s), {nbytes} bytes scanned, {hits} hit(s) {counts or ''}")
            for d in details:
                address = f", VA 0x{d['va']:x}" if 'va' in d else ""
                note = "" if d.get("aligned", True) or d["form"] != "le_u32" else " (unaligned: likely a collision)"
                print(f"   {d['term']} {d['form']}: {d['file']} offset 0x{d['offset']:x}{address}{note}")
            print("   Raw word hits may be float/pointer collisions; use propscan.py for disc property keys.")
            if is_elf:
                print(f"   Code candidates: lui + ori/addiu within {BUILD_WINDOW} instructions only; other builds are not covered.")
    return rec


def write_receipt(rec, card=None):
    """Stamp + write the receipt; returns its id (8 hex)."""
    if not card:
        try:
            card = json.loads(os.environ.get("OPENBF_JOB_JSON", "") or "{}").get("card") or json.loads(os.environ.get("OPENBF_JOB_JSON", "{}")).get("id")
        except ValueError:
            card = None
    rec["card"] = card or "nocard"
    rec["timestamp"] = datetime.datetime.now().isoformat(timespec="seconds")
    rid = hashlib.sha1(json.dumps(rec, sort_keys=True).encode()).hexdigest()[:8]
    rec["id"] = rid
    d = os.path.join(loop_dir(), "receipts")
    os.makedirs(d, exist_ok=True)
    json.dump(rec, open(os.path.join(d, f"{re.sub(r'[^A-Za-z0-9_-]', '', rec['card'])}_{rid}.json"), "w"), indent=1)
    return rid


def absent(c0, terms):
    card, srcs, elf, ts, it = None, [], False, [], iter(terms)
    pc = False
    for x in it:
        if x == "--card":
            card = next(it, None)
        elif x == "--src":
            srcs.append(next(it, ""))
        elif x == "--elf":
            elf = True
        else:
            ts.append(x)
    rec = search(ts, srcs, elf, pc=pc)
    rid = write_receipt(rec, card)
    print(f"[receipt:{rid}]  {len(ts)} term(s), {rec['total_hits']} hit(s) in all sources — "
          f"{os.path.relpath(loop_dir(), ROOT)}/receipts/{rec['card']}_{rid}.json")


def writers(c, terms):
    for t in terms:
        pat = re.compile(r"(^|[^=!<>\w])" + re.escape(t) + r"(\s*\[[^\]]*\])?\s*(=[^=]|\+=|-=|\|=|&=|\^=)")
        hits = [(i + 1, l.strip()) for i, l in enumerate(lines(c)) if pat.search(l)]
        print(f"writes of {t} in {c.upper()}: {len(hits)}")
        for n, l in hits:
            print(f"   {c.upper()}:{n} in {enclosing(c, n)[1]}: {l[:150]}")
        print("   (pointer writes through aliases are not visible to a text search — say so if the claim needs them)")


def word(c, va, n=1, kind="u32"):
    va = int(va, 16) if isinstance(va, str) else va
    off = va - ELF_VA_BASE + ELF_FILE_BASE
    data = open(os.path.join(ROOT, ELF[c]), "rb").read()
    print(f"{ELF[c]} va 0x{va:x} -> file offset 0x{off:x} (off = va - 0x160000 + 0x1000)")
    if kind == "str":
        s = data[off:off + 256].split(b"\0")[0]
        print(f"   string: {s.decode('latin-1')!r}")
        return
    for k in range(int(n)):
        w = data[off + 4 * k: off + 4 * k + 4]
        u = struct.unpack("<I", w)[0]
        f = struct.unpack("<f", w)[0]
        print(f"   0x{va + 4 * k:x}: u32 0x{u:08x} ({u})   f32 {f!r}")


def callers(c, names):
    for nm in names:
        pat = re.compile(re.escape(nm) + r"\s*\(")
        hits = [i + 1 for i, l in enumerate(lines(c)) if pat.search(l) and not HDR.match(l)]
        own = [h for h in hits if enclosing(c, h)[1] != nm]
        print(f"calls of {nm} in {c.upper()}: {len(own)}")
        for n in own:
            print(f"   {c.upper()}:{n} in {enclosing(c, n)[1]}: {lines(c)[n - 1].strip()[:140]}")
        print("   (calls through function pointers / vtables are not visible here — search the address as a data word too)")


def hash_name(name):
    from annotate_hashes import MARKER, read_annotated
    # Validate BOTH inputs before printing anything that could look complete.
    from pathlib import Path
    annotated = {c: read_annotated(Path(ROOT) / CORP[c]) for c in CORP}
    for c, rows in annotated.items():
        hits = []
        for n, row in enumerate(rows, 1):
            for marker in MARKER.finditer(row):
                if name.casefold() in {s.casefold() for s in json.loads(marker.group(1))}:
                    hits.append(n)
                    break
        print(f"{name!r} in {c.upper()}: {len(hits)} annotated line(s) "
              f"in {len({enclosing(c, n)[1] for n in hits})} function(s)")
        fn(c, hits)
    print("(candidate numeric matches; inspect the function to establish a property read)")


ABSENCE_RES = [re.compile(x, re.I) for x in (
    r"\bno class(es)?\b", r"\bnone of the class", r"\bno (class|file|chunk|record|script|function|code|caller|writer|reader|author)s?\b[^.\n]{0,30}\b(author|write|writes|set|sets|read|reads|call|calls|use|uses|contain|contains|reference|references)",
    r"\bnone (author|write|set|call|use|contain)", r"\babsent\b", r"\bnever (authored|authors|written|written to|set|called|read|used|referenced)\b",
    r"\bnot present\b", r"\bnothing (authors|writes|sets|calls|reads|uses|references)\b", r"\bno (callers?|writers?|hits?|users?)\b",
    r"\bdoes not (exist|appear) (anywhere|in any)\b")]
RECEIPT_TAG = re.compile(r"\[receipt:([0-9a-f]{6,16})\]")


def receipt_exists(rid):
    for d in {os.path.join(loop_dir(), "receipts"), os.path.join(ROOT, "engine/artifacts/loop/receipts")}:
        if glob.glob(os.path.join(d, f"*{rid}.json")):
            return True
    return False


def absence_claims(text, only=None):
    """[(line no, text, receipt ids, missing ids)] for each absence-claim line outside code fences. A receipt tag counts on
    the claim's line or the next 3 lines of its paragraph. `only` = set of line numbers to look at (None = all)."""
    L, out, fence = text.split("\n"), [], False
    for i, ln in enumerate(L):
        if ln.lstrip().startswith("```"):
            fence = not fence
            continue
        if fence or (only is not None and i + 1 not in only) or not any(r.search(ln) for r in ABSENCE_RES):
            continue
        ids = []
        for j in range(i, min(i + 4, len(L))):
            if j > i and not L[j].strip():
                break
            ids += RECEIPT_TAG.findall(L[j])
        out.append((i + 1, ln.strip(), ids, [r for r in ids if not receipt_exists(r)]))
    return out


def added_lines(doc, base, repo=None):
    """Line numbers of `doc` added since `base` (all of them when the file is untracked at base)."""
    repo = repo or ROOT
    rel = os.path.relpath(os.path.abspath(doc), repo)
    if subprocess.run(["git", "-C", repo, "cat-file", "-e", f"{base}:{rel}"], capture_output=True, text=True).returncode:
        return None
    nums = set()
    for h in re.finditer(r"^@@ -\S+ \+(\d+)(?:,(\d+))? @@", subprocess.run(["git", "-C", repo, "diff", "-U0", base, "--", rel], capture_output=True, text=True).stdout, re.M):
        a, n = int(h.group(1)), int(h.group(2) or 1)
        nums.update(range(a, a + n))
    return nums


def check_absence(doc, base=None, repo=None):
    """Returns (failures, report lines). A failure = an absence claim with no receipt tag, or a tag naming no receipt."""
    text = open(doc, errors="replace").read()
    only = added_lines(doc, base, repo) if base else None
    bad, rep = 0, []
    for n, t, ids, missing in absence_claims(text, only):
        if not ids:
            bad += 1; rep.append(f"{doc}:{n}: absence claim without a [receipt:<id>] — run `re_check.py absent …` and cite it: {t[:110]}")
        elif missing:
            bad += 1; rep.append(f"{doc}:{n}: receipt {', '.join(missing)} does not exist in {os.path.relpath(os.path.join(loop_dir(), "receipts"), ROOT)}/")
    return bad, rep


def main():
    a = sys.argv[1:]
    if a and a[0] == "check-absence":
        import argparse
        parser = argparse.ArgumentParser(description="Validate absence-claim receipts")
        parser.add_argument("doc")
        parser.add_argument("--warn", action="store_true")
        parser.add_argument("--base")
        args = parser.parse_args(a[1:])
        bad, report = check_absence(args.doc, args.base)
        for line in report:
            print(line)
        sys.exit(0 if args.warn else int(bool(bad)))
    if len(a) == 2 and a[0] == "hash":
        try:
            hash_name(a[1])
        except (OSError, ValueError, KeyError) as exc:
            print(f"hash query incomplete: {exc}", file=sys.stderr)
            sys.exit(2)
        return
    if len(a) < 3 or a[0] not in ("fn", "absent", "writers", "word", "callers") or a[1] not in CORP:
        print(__doc__); sys.exit(1)
    cmd, c, rest = a[0], a[1], a[2:]
    if cmd == "fn":
        fn(c, rest)
    elif cmd == "absent":
        absent(c, rest)
    elif cmd == "writers":
        writers(c, rest)
    elif cmd == "callers":
        callers(c, rest)
    elif cmd == "word":
        kind = "str" if "--str" in rest else "u32"
        nums = [x for x in rest if not x.startswith("--")]
        word(c, nums[0], nums[1] if len(nums) > 1 else 1, kind)


if __name__ == "__main__":
    main()
