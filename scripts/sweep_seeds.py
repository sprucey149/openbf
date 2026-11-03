#!/usr/bin/env python3
"""The round sweep over several seeds (card SWPN): one seed is a noisy instrument.

  scripts/sweep_seeds.py --out <dir> [--seeds 1-8] [--ticks 60000] [--maps bf2_kam1 …]
                         [--target <cargo target dir>] [--jobs N] [--threads N]
                         [--baseline <dir>] [--check <sweep_main_1.txt>] [--reuse]
  scripts/sweep_seeds.py --out <dir> --report-only [--baseline <dir>]   # re-read a finished run

Runs the release `examples/round_to_outcome <ticks> <seed> [maps]` once per seed, the seeds in parallel (`--jobs`,
default all of them; each process gets `SWEEP_THREADS=--threads`, default the CPU count), the whole batch under ONE
hold of `engine/artifacts/loop/ci.lock`. Build the example first (`cargo build --release -p game --example round_to_outcome`).
Each seed keeps its raw output: `sweep_s<seed>.txt` (stdout — byte-identical to a plain run) and `sweep_s<seed>.err`
(stderr, with `STUCK_CENSUS=1` and `SWEEP_DEATHS=1`, which only read the sim). Then `seeds.md`:

  per map   finish count n/N, the winner split (team 0 / team 1), the median finishing tick, stuck_max min / median / max
            over the seeds, the no-progress units alive at the end (the census's "on one leg" count — what the
            stuck_max footer misses; summed over seeds), and the deaths summed over seeds: kills vs the world (falls,
            death regions, vehicles, blasts, the kill floor, landings under the map, lost bots, stalled bots,
            confirmed Respawn and hero time exhaustion without a credited attacker) and the
            world's share; `recycled` = the last resort's uses (card RCYC: bots given up for no net progress).
  footer    maps finishing >= 7/8 of the seeds, maps with any stuck_max >= 10,000, all rounds finished, recycles.
  recycles  every recycle by spot (6 m cells) per map, with the seeds, legs and steering — a spot on 2+ seeds is a
            defect for the card that owns it; the number the stall cards drive to 0.
  shares    per map: walking in place (moving > 1 m/s, under 1 m net in 10 s), off-graph (locus N, of the units on
            foot since card FOFG), seated (in a vehicle or turret seat), slide and ledge shares of unit-seconds, with the biggest walking-in-place cells — what the stuck census cannot see; and
            `rim` (card BOWL): the share on a hull past a rim rail (beyond a barrier from the three nearest hubs and
            0.3 m or more under their floor), how many of those the level test still calls on, the longest stay;
            and (card CONT) the share with an enemy within 20 m and the on-foot unit-seconds per target change;
            and (card MOVR) B9, unexplained standing: the share of on-foot AI time inside a spell of 20 s or more
            within 2 m with no target and a behaviour that has no reason to stand, split by behaviour.
  bot quality  with `--botq` (card BOTM): per map, how the bots MOVE - reversals per 10 s, path / net, staying and
            pinball windows, spacing, contact, team kills, map use - the PS2's live-RAM numbers beside the port's
            (`scripts/bot_quality.py`, `analysis/bot-quality.md`).

Medians are LOWER medians (a value some seed produced), as the example's own footer. `--baseline <dir>` (a previous
run's out dir, or any dir of `sweep_s<seed>.txt` [+ `.err`]) adds per-map deltas over the seeds both runs have.
`--check <file>` (default: gates.toml's baseline_dir/sweep_main_1.txt, when seed 42 is in the run) compares seed 42's
stdout with the gate's baseline row by row — the instrument check. `--reuse` keeps only matching successful receipts with intact output artifacts.
Exit: 0 ok, 1 a seed's run failed, 2 the seed-42 check differs.
"""
import argparse
import fcntl
import hashlib
import json
import tempfile
import os
import re
import subprocess
import sys
import time
import tomllib
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# The main checkout (the parent of the shared .git), also when this runs from a worktree.
MAIN_ROOT = os.path.dirname(subprocess.run(["git", "-C", ROOT, "rev-parse", "--path-format=absolute", "--git-common-dir"],
                                           capture_output=True, text=True, check=True).stdout.strip())
LOCK = os.path.join(MAIN_ROOT, "engine/artifacts/loop/ci.lock")
GATES_TOML = os.path.join(MAIN_ROOT, "engine/artifacts/loop/gates.toml")

NOPROG = re.compile(r"^census (\S+): (\d+) alive units on one leg")
# Since KFLR (2026-10-01) each team's row may carry three more causes: floor (the kill floor), under (a landing under
# the map) and lost (a bot off the playable level with no way back), and since RCYC a ninth: stalled (a bot given up
# for no net progress). Hero health adds suicide and hero_exhausted. Preserve
# the historical five/eight/nine-cause rows by padding their missing counters.
_TEAM = (r"kill (\d+) region (\d+) fall (\d+) vehicle (\d+) blast (\d+)"
         r"(?: floor (\d+) under (\d+) lost (\d+))?(?: stalled (\d+))?"
         r"(?: suicide (\d+) hero_exhausted (\d+))?")
DEATHS = re.compile(r"^deaths (\S+): team0 " + _TEAM + r" \| team1 " + _TEAM)
DEATH_CAUSES = ("kill", "region", "fall", "veh", "blast", "floor", "under", "lost", "stalled",
                "suicide", "hero_exhausted")
NCAUSE = len(DEATH_CAUSES)
STALLED = DEATH_CAUSES.index("stalled")
RECYCLE = re.compile(r"^recycle (\S+): seed (\d+) tick (\d+) unit (\d+) gen (\d+) team (\d+) class (\d+) "
                     r"pos \(([-\d.]+),([-\d.]+),([-\d.]+)\) leg (.*?)(?: wp (\S+))? locus (\S+) floor_dy (\S+) steer (.*?) objective (.*)$")
# Card STLVL: the same recycle against the leg hub's RESOLVED floor (`floor_dy` reads a roof as the floor).
RECYCLE_LV = re.compile(r"^recycle_lv (\S+): seed (\d+) tick (\d+) unit (\d+) hub (\d+) hub_floor (\S+) feet_dy (\S+) "
                        r"hub_d (\S+) hub_r (\S+) ")
SHARES = re.compile(r"^shares (\S+): unit-s (\d+) \| walk-in-place (\d+) \([\d.]+ %\) \| locus H ([\d.]+) % A ([\d.]+) % "
                    r"N ([\d.]+) % \| steer idle ([\d.]+) fwd ([\d.]+) slide ([\d.]+) fan ([\d.]+) back ([\d.]+) "
                    r"ledge ([\d.]+) % \| slide-in-N ([\d.]+) % "
                    r"(?:\| rim (\d+) \([\d.]+ %\) on-level (\d+) longest (\d+) s )?"
                    r"(?:\| contact20 ([\d.]+) % \| retarget (\d+) in (\d+) foot-s )?"
                    r"(?:\| stand20 (\d+) in (\d+) foot-s \([\d.]+ %\) \[([^\]]*)\] )?"
                    r"(?:\| seated (\d+) \([\d.]+ %\) foot (\d+) )?\| top (.*)$")
CELL = 6.0


def parse_seeds(spec):
    out = []
    for part in spec.split(","):
        a, _, b = part.partition("-")
        out += list(range(int(a), int(b) + 1)) if b else [int(a)]
    return out


def lower_median(v):
    v = sorted(v)
    return v[(len(v) - 1) // 2] if v else None


def parse_stdout(path):
    """{map: dict(finished, winner, tick, stuck_max, line)} for every result row (map rows start in column 0).

    Only scoreboard rows count. Diagnostics (including column-zero flyer rows)
    must not masquerade as maps or hide a missing selected-map result."""
    rows = {}
    for line in open(path, errors="replace").read().splitlines():
        if not re.match(r"^\S+\s+(?:team [01] wins at tick \d+|no outcome\b)", line, re.I):
            continue
        key = line.split()[0]
        m = re.search(r"team (\d+) wins at tick (\d+)", line)
        s = re.search(r"stuck_max (\d+)", line)
        rows[key] = dict(finished=bool(m), winner=int(m.group(1)) if m else None, tick=int(m.group(2)) if m else None,
                         stuck_max=int(s.group(1)) if s else None, line=line)
    return rows


def parse_stderr(path):
    """{map: dict(noprog, deaths in DEATH_CAUSES order, summed over both teams)}"""
    out = {}
    if not os.path.exists(path):
        return out
    for line in open(path, errors="replace").read().splitlines():
        m = NOPROG.match(line)
        if m:
            out.setdefault(m.group(1), {})["noprog"] = int(m.group(2))
        m = DEATHS.match(line)
        if m:
            n = [int(x or 0) for x in m.groups()[1:]]
            out.setdefault(m.group(1), {})["deaths"] = [n[i] + n[i + NCAUSE] for i in range(NCAUSE)]
        m = RECYCLE.match(line)
        if m:
            g = m.groups()
            out.setdefault(g[0], {}).setdefault("recycles", []).append(dict(
                seed=int(g[1]), tick=int(g[2]), unit=int(g[3]), team=int(g[5]), pos=tuple(float(v) for v in g[7:10]),
                leg=g[10], wp=g[11], locus=g[12], floor_dy=g[13], steer=g[14], objective=g[15]))
        m = RECYCLE_LV.match(line)
        if m and m.group(7) != "-":
            # Another storey: at the leg's hub (within its radius of the centre, at least 6 m and at most 10 m: a
            # 500 m hub on rolling terrain is no storey) and > 3 m off its floor.
            g = m.groups()
            if abs(float(g[6])) > 3.0 and float(g[7]) <= max(min(float(g[8]), 10.0), 6.0):
                out.setdefault(g[0], {}).setdefault("storey", []).append((int(g[1]), int(g[2]), int(g[3]), int(g[4])))
        m = SHARES.match(line)
        if m:
            g = m.groups()
            tops = {c.rsplit(":", 1)[0]: int(c.rsplit(":", 1)[1]) for c in g[24].split() if ":" in c}
            # Since MOVR: B9, unexplained standing (unit-s in a 20 s spell within 2 m, no target, a behaviour with
            # no reason to stand), the on-foot AI unit-s, and the split by behaviour.
            stand = None if g[19] is None else (int(g[19]), int(g[20]), {
                b.rsplit(" ", 1)[0]: int(b.rsplit(" ", 1)[1]) for b in g[21].split(", ") if b})
            # Since BOWL: rim = unit-seconds on a hull past a rim (None on older runs), with the longest stay.
            rim = None if g[13] is None else (int(g[13]), int(g[14]), int(g[15]))
            # Since CONT: an enemy within 20 m (% of unit-s), on-foot target changes and on-foot unit-s.
            cont = None if g[16] is None else (float(g[16]), int(g[17]), int(g[18]))
            # Since FOFG: the locus (N) is of the units on foot (`foot` samples); `seated` = samples in a seat.
            # Older runs counted every alive unit: foot = n there.
            seated, foot = (None, int(g[1])) if g[22] is None else (int(g[22]), int(g[23]))
            out.setdefault(g[0], {})["shares"] = dict(n=int(g[1]), wip=int(g[2]), N=float(g[5]), slide=float(g[8]),
                                                       ledge=float(g[11]), slide_in_n=float(g[12]), tops=tops, rim=rim, cont=cont,
                                                       stand=stand, seated=seated, foot=foot)
    return out


def load_run(d, seeds=None):
    """{seed: (stdout rows, stderr data)} for every sweep_s<seed>.txt in d (restricted to `seeds` if given)."""
    run = {}
    for f in os.listdir(d):
        m = re.fullmatch(r"sweep_s(\d+)\.txt", f)
        if m and (seeds is None or int(m.group(1)) in seeds):
            s = int(m.group(1))
            run[s] = (parse_stdout(os.path.join(d, f)), parse_stderr(os.path.join(d, f"sweep_s{s}.err")))
    return run


def per_map(run):
    """{map: stats} over the seeds of `run`, map order = first seed's row order."""
    order = []
    for s in sorted(run):
        for k in run[s][0]:
            if k not in order:
                order.append(k)
    stats = {}
    for k in order:
        rows = [run[s][0][k] for s in sorted(run) if k in run[s][0]]
        extra = [run[s][1].get(k, {}) for s in sorted(run) if k in run[s][0]]
        sm = [r["stuck_max"] for r in rows if r["stuck_max"] is not None]
        ticks = [r["tick"] for r in rows if r["finished"]]
        np_ = [e["noprog"] for e in extra if "noprog" in e]
        dl = [e["deaths"] for e in extra if "deaths" in e]
        rc = [r for e in extra for r in e.get("recycles", [])]
        sh = [e["shares"] for e in extra if "shares" in e]
        stats[k] = dict(
            recycles=rc, recycled=len(rc) if any("deaths" in e or "recycles" in e for e in extra) else None,
            storey=[r for e in extra for r in e.get("storey", [])],
            shares=sh,
            n=len(rows), fin=sum(r["finished"] for r in rows),
            w0=sum(r["winner"] == 0 for r in rows), w1=sum(r["winner"] == 1 for r in rows),
            tick=lower_median(ticks), sm=sm, sm_med=lower_median(sm),
            noprog=sum(np_) if np_ else None,
            deaths=[sum(d[i] for d in dl) for i in range(NCAUSE)] if dl else None)
    return stats


def fmt_deaths(d):
    if d is None:
        return "-", "-", "-"
    env = sum(d[1:])
    tot = d[0] + env
    parts = [f"{n} {lab}" for n, lab in zip(d[1:], DEATH_CAUSES[1:]) if n]
    return str(d[0]), f"{env}" + (f" ({', '.join(parts)})" if parts else ""), f"{100 * env / tot:.0f} %" if tot else "-"


def table(stats, nseeds, base=None):
    L = []
    hdr = "| map | finished | wins t0/t1 | median finish tick | stuck_max min / med / max | noprog at end | recycled | kills | world deaths | world % |"
    if base:
        hdr += " Δ finished | Δ stuck_max med |"
    L.append(hdr)
    L.append("|" + "---|" * (hdr.count("|") - 1))
    for k, st in stats.items():
        sm = st["sm"]
        kills, env, pct = fmt_deaths(st["deaths"])
        row = (f"| {k} | {st['fin']}/{st['n']} | {st['w0']}/{st['w1']} | {st['tick'] if st['tick'] is not None else '-'} | "
               f"{min(sm) if sm else '-'} / {st['sm_med'] if sm else '-'} / {max(sm) if sm else '-'} | "
               f"{st['noprog'] if st['noprog'] is not None else '-'} | "
               f"{st['recycled'] if st['recycled'] is not None else '-'} | {kills} | {env} | {pct} |")
        if base:
            b = base.get(k)
            if b:
                df = st["fin"] - b["fin"]
                dm = (st["sm_med"] - b["sm_med"]) if st["sm_med"] is not None and b["sm_med"] is not None else None
                row += f" {df:+d} (was {b['fin']}/{b['n']}) | {'-' if dm is None else f'{dm:+d}'} (was {b['sm_med']}) |"
            else:
                row += " new | new |"
        L.append(row)
    n = len(stats)
    fin_all = sum(st["fin"] for st in stats.values())
    rounds = sum(st["n"] for st in stats.values())
    good = sum(st["fin"] * 8 >= 7 * st["n"] for st in stats.values())
    over = [k for k, st in stats.items() if st["sm"] and max(st["sm"]) >= 10_000]
    L.append("")
    rec = [st["recycled"] for st in stats.values() if st["recycled"] is not None]
    L.append(f"**seeds: {nseeds}  maps finishing >= 7/8 of their seeds {good}/{n}  maps with any stuck_max >= 10k "
             f"{len(over)} ({' '.join(over) if over else '-'})  rounds finished {fin_all}/{rounds}  "
             f"recycled {sum(rec) if rec else '-'}**")
    return L


def cell_of(pos):
    """6 m in XZ, 3 m in y (a floor)."""
    return (int(round(pos[0] / CELL) * CELL), int(round(pos[1] / 3.0) * 3), int(round(pos[2] / CELL) * CELL))


def most(values, k=2):
    """The k most common of `values` as `v xN`, ties by value."""
    c = {}
    for v in values:
        c[v] = c.get(v, 0) + 1
    return ", ".join(f"{v} x{n}" for v, n in sorted(c.items(), key=lambda t: (-t[1], t[0]))[:k])


def recycle_spots(stats):
    """[(map, cell, [recycles])] — every spot a bot was recycled at, most-used first within its map."""
    spots = []
    for k, st in stats.items():
        cells = {}
        for r in st["recycles"]:
            cells.setdefault(cell_of(r["pos"]), []).append(r)
        for c, rs in sorted(cells.items(), key=lambda t: (-len(set(r["seed"] for r in t[1])), -len(t[1]), t[0])):
            spots.append((k, c, rs))
    return spots


def recycle_section(stats):
    total = sum(len(st["recycles"]) for st in stats.values())
    if all(st["recycled"] is None for st in stats.values()):
        return ["(no recycle data: these runs predate card RCYC)"]
    spots = recycle_spots(stats)
    rep = [s for s in spots if len(set(r["seed"] for r in s[2])) >= 2]
    L = [f"**{total} recycles at {len(spots)} spots on {sum(bool(st['recycles']) for st in stats.values())} maps; "
         f"{len(rep)} spots on 2 or more seeds (each a DEFECT for the card that owns it).** A recycle is the last "
         f"resort (`core::ai` RECYCLE_AFTER): a bot that wanted to walk and made no net progress. The target is 0.", ""]
    storey = {k: st.get("storey", []) for k, st in stats.items() if st.get("storey")}
    L += [f"**On another storey than the leg's hub: {sum(len(v) for v in storey.values())}** (at the hub — within its radius, "
          f"6 to 10 m — and more than 3 m off its resolved floor; card STLVL)"
          + (": " + ", ".join(f"{k} {len(v)} (hub{'s' if len(set(r[3] for r in v)) > 1 else ''} "
                              f"{' '.join(str(h) for h in sorted(set(r[3] for r in v)))})" for k, v in sorted(storey.items()))
             if storey else "") + ".", ""]
    if not spots:
        return L
    L += ["| map | spot (cell: 6 m x, 3 m y, 6 m z) | recycles | seeds | teams | leg from->to dest | locus | floor dy | steer | objective |",
          "|---|---|---|---|---|---|---|---|---|---|"]
    for k, c, rs in spots:
        seeds = sorted(set(r["seed"] for r in rs))
        dys = sorted(float(r["floor_dy"]) for r in rs if r["floor_dy"] not in ("NaN", "nan"))
        dy = "-" if not dys else (f"{dys[0]:+.1f}" if dys[0] == dys[-1] else f"{dys[0]:+.1f}..{dys[-1]:+.1f}")
        L.append(f"| {k} | ({c[0]}, {c[1]}, {c[2]}) | {len(rs)} | {'**' if len(seeds) >= 2 else ''}{len(seeds)}"
                 f"{'**' if len(seeds) >= 2 else ''} ({' '.join(map(str, seeds))}) | {most(r['team'] for r in rs)} | "
                 f"{most(r['leg'] for r in rs)} | {most(r['locus'] for r in rs)} | {dy} | {most(r['steer'] for r in rs)} | "
                 f"{most(r['objective'] for r in rs)} |")
    return L


def shares_section(stats):
    rows = [(k, st["shares"]) for k, st in stats.items() if st["shares"]]
    if not rows:
        return ["(no shares data: these runs predate card RCYC)"]
    L = ["| map | walking in place % | off graph (N) % of foot | seated % | slide % | ledge % | slide within N % | rim % (unit-s; level test says on; longest stay s) | enemy within 20 m % | on-foot unit-s per target change | B9 unexplained standing % of foot time (by behaviour, unit-s) | top walking-in-place cells (team(x,y,z):unit-s, summed) |",
         "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for k, sh in rows:
        n = sum(x["n"] for x in sh) or 1
        w = lambda f: sum(x[f] * x["n"] for x in sh) / n
        foot = sum(x["foot"] for x in sh) or 1
        seats = [x["seated"] for x in sh if x["seated"] is not None]
        seated = f"{100 * sum(seats) / n:.1f}" if seats else "-"
        tops = {}
        for x in sh:
            for c, v in x["tops"].items():
                tops[c] = tops.get(c, 0) + v
        top = " ".join(f"{c}:{v}" for c, v in sorted(tops.items(), key=lambda t: (-t[1], t[0]))[:3]) or "-"
        rims = [x["rim"] for x in sh if x.get("rim")]
        rim = "-"
        if rims:
            r = sum(v[0] for v in rims)
            rim = f"{100 * r / n:.2f} ({r}; {sum(v[1] for v in rims)}; {max(v[2] for v in rims)})"
        conts = [(x["cont"], x["n"]) for x in sh if x.get("cont")]
        c20 = per = "-"
        if conts:
            c20 = f"{sum(c[0] * m for c, m in conts) / (sum(m for _, m in conts) or 1):.1f}"
            ch = sum(c[1] for c, _ in conts)
            per = f"{sum(c[2] for c, _ in conts) / ch:.0f} ({ch})" if ch else "- (0)"
        stands = [x["stand"] for x in sh if x.get("stand")]
        b9 = "-"
        if stands:
            by = {}
            for v in stands:
                for b, c in v[2].items():
                    by[b] = by.get(b, 0) + c
            split = ", ".join(f"{b} {c}" for b, c in sorted(by.items(), key=lambda t: -t[1])) or "-"
            b9 = f"{100 * sum(v[0] for v in stands) / (sum(v[1] for v in stands) or 1):.2f} ({split})"
        L.append(f"| {k} | {100 * sum(x['wip'] for x in sh) / n:.1f} | {sum(x['N'] * x['foot'] for x in sh) / foot:.1f} | {seated} | {w('slide'):.1f} | {w('ledge'):.1f} | "
                 f"{w('slide_in_n'):.1f} | {rim} | {c20} | {per} | {b9} | {top} |")
    return L


def check_42(out, check, subset):
    """Seed 42's stdout vs the gate baseline, row by row (only the run's maps when `subset`). Returns (ok, lines)."""
    got = open(os.path.join(out, "sweep_s42.txt")).read().splitlines()
    want = open(check).read().splitlines()
    rows_g = {k: r["line"] for k, r in parse_stdout(os.path.join(out, "sweep_s42.txt")).items()}
    rows_w = {k: r["line"] for k, r in parse_stdout(check).items() if not subset or k in rows_g}
    diff = [k for k in rows_w.keys() | rows_g.keys() if rows_w.get(k) != rows_g.get(k)]
    whole = (got == want) if not subset else not diff
    lines = [f"seed 42 vs `{check}`: {('byte-identical' if not subset else 'rows identical') if whole else 'DIFFERS'}; "
             f"{len(rows_w) - len([k for k in diff if k in rows_w])}/{len(rows_w)} rows identical"]
    lines += [f"  moved: {k}: `{rows_g.get(k)}` (was `{rows_w.get(k)}`)" for k in sorted(diff)]
    return whole, lines


def file_hash(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def completed_artifacts(out, seed, maps, botq):
    """Validate the executable's result inventory, including unfinished rounds.

    Default selection is discovered by the executable: its `done (N/M, ...)`
    stderr lines carry all selected map keys, including failed loads.
    """
    o, e = f"sweep_s{seed}.txt", f"sweep_s{seed}.err"
    rows = parse_stdout(os.path.join(out, o))
    with open(os.path.join(out, e)) as f:
        done = re.findall(r"^(\S+) done \((\d+)/(\d+),", f.read(), re.M)
    expected = set(maps) if maps else {m for m, _, _ in done}
    if not expected or (not maps and (len(done) != len(expected) or
            any(int(n) != len(expected) for _, _, n in done) or
            {int(i) for _, i, _ in done} != set(range(1, len(expected) + 1)))):
        raise ValueError("missing/incomplete selected-map inventory")
    if set(rows) != expected or any(rows[m]["stuck_max"] is None for m in expected):
        raise ValueError("missing/incomplete result rows for selected maps")
    with open(os.path.join(out, o)) as f:
        footer = re.search(r"^sweep: finished \d+/(\d+)  stuck_max median ", f.read(), re.M)
    if not footer or int(footer[1]) != len(expected):
        raise ValueError("missing/incomplete sweep scoreboard")
    artifacts = [o, e]
    if botq:
        artifacts += [f"botq/{m}_s{seed}{suffix}" for m in sorted(expected) for suffix in (".csv", ".ev.csv")]
    if any(os.path.getsize(os.path.join(out, f)) == 0 for f in artifacts if f not in (e,)):
        raise ValueError("empty required artifact")
    return {f: file_hash(os.path.join(out, f)) for f in artifacts}


def reusable(receipt, identity, out):
    try:
        with open(receipt) as f:
            rec = json.load(f)
        return (rec["identity"] == identity and rec["artifacts"] ==
                completed_artifacts(out, identity["seed"], identity["maps"], identity["botq"]))
    except (OSError, ValueError, KeyError, TypeError):
        return False


def write_completion(receipt, identity, artifacts):
    # Same-directory replacement: an interrupted write never becomes success.
    fd, tmp = tempfile.mkstemp(prefix=".sweep-receipt-", dir=os.path.dirname(receipt))
    try:
        with os.fdopen(fd, "w") as f:
            json.dump(dict(identity=identity, artifacts=artifacts), f, sort_keys=True)
        os.replace(tmp, receipt)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", required=True)
    ap.add_argument("--seeds", default="1-8")
    ap.add_argument("--ticks", type=int, default=60000)
    ap.add_argument("--maps", nargs="*", default=[])
    ap.add_argument("--target", default=os.path.join(ROOT, "engine/target"))
    ap.add_argument("--jobs", type=int, default=0, help="seeds run at once (default: all)")
    ap.add_argument("--threads", type=int, default=os.cpu_count(), help="SWEEP_THREADS per seed")
    ap.add_argument("--baseline")
    ap.add_argument("--check")
    ap.add_argument("--reuse", action="store_true")
    ap.add_argument("--report-only", action="store_true")
    ap.add_argument("--botq", action="store_true",
                    help="bot-quality samples (card BOTM): BOT_SAMPLES=<out>/botq for the first 540 s of every round, "
                         "and a table per map in seeds.md, the PS2's numbers beside the port's")
    ap.add_argument("--no-botq-report", action="store_true",
                    help="collect and validate bot samples without repeating their analysis in seeds.md")
    a = ap.parse_args()
    out = os.path.abspath(a.out)
    os.makedirs(out, exist_ok=True)
    seeds = parse_seeds(a.seeds)
    exe = os.path.join(os.path.abspath(a.target), "release/examples/round_to_outcome")
    wall, rcs = None, {}
    if not a.report_only:
        if not os.access(exe, os.X_OK):
            sys.exit(f"sweep_seeds: no {exe} — build it: cargo build --release -p game --example round_to_outcome")
        env = dict(os.environ, STUCK_CENSUS="1", SWEEP_DEATHS="1", SWEEP_THREADS=str(a.threads))
        if a.botq:
            env.update(BOT_SAMPLES=os.path.join(out, "botq"), BOT_SAMPLES_SECS="540")

        binary = file_hash(exe)
        env_hash = hashlib.sha256(json.dumps(env, sort_keys=True).encode()).hexdigest()

        def one(seed):
            o, e = os.path.join(out, f"sweep_s{seed}.txt"), os.path.join(out, f"sweep_s{seed}.err")
            receipt = os.path.join(out, f"sweep_s{seed}.complete.json")
            identity = dict(version=1, binary=binary, executable=os.path.realpath(exe), seed=seed,
                            ticks=a.ticks, maps=a.maps, threads=a.threads, botq=a.botq, environment=env_hash)
            if a.reuse and reusable(receipt, identity, out):
                return 0
            # Invalidate success before truncating any previous output.
            if os.path.exists(receipt):
                os.unlink(receipt)
            if a.botq:
                import glob
                for old in glob.glob(os.path.join(out, "botq", f"*_s{seed}.csv")) + glob.glob(os.path.join(out, "botq", f"*_s{seed}.ev.csv")):
                    os.unlink(old)
            t0 = time.time()
            with open(o, "w") as fo, open(e, "w") as fe:
                rc = subprocess.call([exe, str(a.ticks), str(seed)] + a.maps, cwd=os.path.join(ROOT, "engine"),
                                     env=env, stdout=fo, stderr=fe)
            if rc == 0:
                try:
                    artifacts = completed_artifacts(out, seed, a.maps, a.botq)
                    if file_hash(exe) != binary:
                        raise ValueError("binary changed during run")
                    write_completion(receipt, identity, artifacts)
                except (OSError, ValueError) as exc:
                    print(f"[sweep_seeds] seed {seed}: incomplete output: {exc}", file=sys.stderr)
                    rc = 1
            print(f"[sweep_seeds] seed {seed}: exit {rc}, {time.time() - t0:.0f} s", flush=True)
            return rc

        with open(LOCK, "a") as lk:
            print(f"[sweep_seeds] waiting for {LOCK}", flush=True)
            fcntl.flock(lk, fcntl.LOCK_EX)
            t0 = time.time()
            with ThreadPoolExecutor(max_workers=a.jobs or len(seeds)) as ex:
                rcs = dict(zip(seeds, ex.map(one, seeds)))
            wall = time.time() - t0
    run = load_run(out, set(seeds))
    missing = [s for s in seeds if s not in run]
    if missing:
        sys.exit(f"sweep_seeds: no output for seeds {missing}")
    stats = per_map(run)
    base = None
    if a.baseline:
        brun = load_run(a.baseline, set(seeds))
        base = per_map(brun)
    commit = subprocess.run(["git", "-C", ROOT, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
    L = [f"# Round sweep over {len(seeds)} seeds — {time.strftime('%Y-%m-%d %H:%M')}", "",
         f"- tree `{ROOT}` at `{commit}`; binary `{exe}`",
         f"- `round_to_outcome {a.ticks} <seed>{' ' + ' '.join(a.maps) if a.maps else ''}`, seeds {a.seeds} "
         f"({' '.join(map(str, seeds))}); STUCK_CENSUS=1 SWEEP_DEATHS=1 (stderr only)",
         f"- wall time: {f'{wall:.0f} s ({a.jobs or len(seeds)} seeds at once, SWEEP_THREADS={a.threads})' if wall is not None else 'not run here (--report-only)'}"]
    if rcs and any(rcs.values()):
        L.append(f"- **FAILED seeds: {[s for s, r in rcs.items() if r]}** (see sweep_s<seed>.err)")
    if base:
        L.append(f"- baseline: `{a.baseline}` (Δ columns, over the same seeds; its rounds are those files)")
    rc = 1 if rcs and any(rcs.values()) else 0
    check = a.check
    if check is None and 42 in seeds:
        try:
            check = os.path.join(tomllib.load(open(GATES_TOML, "rb"))["baseline_dir"], "sweep_main_1.txt")
        except (OSError, KeyError):
            check = None
    if check and 42 in seeds:
        ok, lines = check_42(out, check, bool(a.maps))
        L += ["", "## Instrument check"] + lines
        if not ok and not rc:
            rc = 2
    L += ["", "## Per map", ""] + table(stats, len(seeds), base)
    L += ["", "Deaths are summed over the seeds and both teams: kills = `Event::Kill`, world = `Event::EnvDeath` "
          "(region / fall / veh = a destroyed vehicle's rider / blast = an attackerless blast / stalled = recycled / "
          "suicide = confirmed Respawn / hero_exhausted = hero time drain with no credited attacker). "
          "noprog = alive units on one leg >= NO_PROGRESS_TICKS at the round's end, summed over the seeds; recycled = "
          "bots the last resort gave up, summed over the seeds."]
    L += ["", "## Recycles", ""] + recycle_section(stats)
    L += ["", "## Walking in place, off graph, sliding (shares of alive unit-seconds, over the seeds)", ""] + shares_section(stats)
    if os.path.isdir(os.path.join(out, "botq")) and not a.no_botq_report:
        import bot_quality
        L += ["", "## Bot quality (card BOTM; 4 Hz samples, 40 - 540 s of each round; `scripts/bot_quality.py`)", ""]
        L += bot_quality.seeds_section(os.path.join(out, "botq"))
    open(os.path.join(out, "seeds.md"), "w").write("\n".join(L) + "\n")
    print("\n".join(L))
    sys.exit(rc)


if __name__ == "__main__":
    main()
