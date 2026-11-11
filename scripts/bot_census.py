#!/usr/bin/env python3
"""bot_census.py - the bot rework's census and proxy-gate tables (analysis/bot-rework-plan.md, stage 0).

Reads what round_to_outcome already prints; it runs nothing.

  scripts/bot_census.py census A [B]          sweep out folders (sweep_s<seed>.txt + .err, the layout scripts/sweep_seeds.py
                                              writes; the .err needs STUCK_CENSUS, SWEEP_DEATHS=1 and BOT_CENSUS=1)
  scripts/bot_census.py compare BASE CAND [--noise DIR]
  scripts/bot_census.py dc ERR [--seeds 1-16] [--maps a b]
                                              a --difficulty-check stderr: proxy wins per tier, side and map with Wilson
                                              95 % intervals, the D1 line and the levers per foot-minute

census metrics, each a ratio of sums over the block's maps and seeds:
  recycles, relocations     `recycle` / `relocate` lines
  stand20                   `shares`: foot-s inside a 20 s stand with no reason (B9), and its share of foot-s
  recover %                 `behave`: recover foot-s over foot-s
  B12 cover %               `behave`: fight time at a position with cover from the target
  kills/min, kills/foot-min `deaths` kill column over round minutes (stdout's finishing tick, else the tick limit),
                            and over foot-minutes (card HYST's figure)
  flips/fight-min           `hyst` flips over `behave` fight minutes (card HYST)
  d1 bot far %              `d1`: thinks on a bot target with an engageable enemy bot at most half as far
  levers/foot-min           `levers <map> <group>`: per lever, summed over the groups
  think p50/p99             `think`: the per-call and per-think-tick quantiles, the median over rounds (machine time)
  timing TIME_FILE [--base main --cand branch --budget 5]
                                              the think-cost check: user seconds of alternating pinned sweeps
                                              ("<who> <n> wall <s> s user <s> s" lines) against a percentage budget
  selftest                                    check the noise band on simulated re-rolls (no files)

The noise band answers the question stage 1 asks: is a candidate block's value further from the base block's than two
blocks of the same code differ when the rounds re-roll? For blocks of n_a and n_b seeds that difference has standard
deviation sd * sqrt(1/n_a + 1/n_b), sd being the per-seed spread (stdev of the per-seed values over the base blocks),
and the band is twice that: about 95 % of pure re-rolls land inside it. For ratio metrics the per-seed ratio stands in
for the block ratio. census with two folders gives each block, their difference and the noise: max(|A - B|, the band
for an A-sized block against a B-sized one). compare marks each metric of CAND against BASE inside or OUTSIDE
max(band for BASE against CAND, |BASE - NOISE|), the spread taken over BASE and --noise (a second base block).
"""
import argparse
import glob
import math
import os
import re
import statistics
import sys

RECYCLE = re.compile(r"^recycle (\S+): seed ")
RELOCATE = re.compile(r"^relocate (\S+): seed ")
STAND20 = re.compile(r"^shares (\S+): .*\| stand20 (\d+) in (\d+) foot-s")
BEHAVE = re.compile(r"^behave (\S+): foot-s (\d+) \| (.*?) \| B12 cover (\d+) at-position (\d+) of fight (\d+)")
HYST = re.compile(r"^hyst (\S+): flips (\d+) kept (\d+)")
DEATHS = re.compile(r"^deaths (\S+): team0 kill (\d+) .*\| team1 kill (\d+) ")
D1 = re.compile(r"^d1 (\S+) (.+?): thinks (\d+) \| human (\d+) far (\d+) \([\d.]+ %\) takes (\d+) far (\d+) \| "
                r"bot (\d+) far (\d+) \([\d.]+ %\) takes (\d+) far (\d+)$")
LEVERS = re.compile(r"^levers (\S+) (.+?): foot-min ([\d.]+) thinks (\d+) \| (.*) \| events (.*)$")
THINK = re.compile(r"^think (\S+): calls (\d+) p50 ([\d.]+) p99 ([\d.]+) max ([\d.]+) us \| thinks (\d+) p50 ([\d.]+) "
                   r"p99 ([\d.]+) \| proxy calls (\d+) p50 ([\d.]+) p99 ([\d.]+)")
ROUND = re.compile(r"^round (\S+) s(\d+) (\S+) winner (\S+) margin (-?\d+) tickets \S+(?:\s+proxy (\d+) \S+ kills (\d+) "
                   r"deaths (\d+))?")
MAPROW = re.compile(r"^(\S+)\s+(?:team ([01]) wins at tick (\d+)|no outcome\b)", re.I)
TICK_HZ = 30
D1_KEYS = ("thinks", "human", "human_far", "human_takes", "human_takes_far", "bot", "bot_far", "bot_takes",
           "bot_takes_far")


def read_lines(path):
    with open(path, errors="replace") as f:
        return f.read().splitlines()


def pairs(text):
    """'a 1 b-c 2' -> {'a': 1, 'b-c': 2}"""
    t = text.split()
    return {t[i]: int(t[i + 1]) for i in range(0, len(t) - 1, 2)}


def blank():
    return dict(recycles=0, relocations=0, stand20=0, foot_s=0, recover=0, b12=0, fight=0, flips=0, kills=0,
                round_ticks=0, rounds=0, finished=0, d1=dict.fromkeys(D1_KEYS, 0), levers={}, lever_foot_min=0.0,
                think=[])


def add_census_lines(acc, lines):
    """Fold one stderr's census lines into acc (any number of maps)."""
    for line in lines:
        if RECYCLE.match(line):
            acc["recycles"] += 1
        elif RELOCATE.match(line):
            acc["relocations"] += 1
        elif m := STAND20.match(line):
            acc["stand20"] += int(m.group(2))
        elif m := BEHAVE.match(line):
            acc["foot_s"] += int(m.group(2))
            acc["recover"] += pairs(m.group(3)).get("recover", 0)
            acc["b12"] += int(m.group(4))
            acc["fight"] += int(m.group(6))
        elif m := HYST.match(line):
            acc["flips"] += int(m.group(2))
        elif m := DEATHS.match(line):
            acc["kills"] += int(m.group(2)) + int(m.group(3))
        elif m := D1.match(line):
            if not m.group(2).startswith("proxy "):
                for k, v in zip(D1_KEYS, m.groups()[2:]):
                    acc["d1"][k] += int(v)
        elif m := LEVERS.match(line):
            if not m.group(2).startswith("proxy "):
                acc["lever_foot_min"] += float(m.group(3))
                for k, v in {**pairs(m.group(5)), **pairs(m.group(6))}.items():
                    acc["levers"][k] = acc["levers"].get(k, 0) + v
        elif m := THINK.match(line):
            acc["think"].append(tuple(float(x) for x in m.groups()[1:]))


def load_seed(txt, err, ticks):
    acc = blank()
    for line in read_lines(txt):
        if m := MAPROW.match(line):
            acc["rounds"] += 1
            acc["finished"] += bool(m.group(2))
            acc["round_ticks"] += int(m.group(3)) if m.group(3) else ticks
    if os.path.exists(err):
        add_census_lines(acc, read_lines(err))
    return acc


def load_dir(d, ticks):
    """{seed: acc} for every sweep_s<seed>.txt in d."""
    out = {}
    for txt in sorted(glob.glob(os.path.join(d, "sweep_s*.txt"))):
        seed = int(re.search(r"sweep_s(\d+)\.txt$", txt).group(1))
        out[seed] = load_seed(txt, txt[:-4] + ".err", ticks)
    if not out:
        sys.exit(f"bot_census: no sweep_s<seed>.txt in {d}")
    return out


def merge(accs):
    t = blank()
    for a in accs:
        for k, v in a.items():
            if k == "d1":
                for j in D1_KEYS:
                    t["d1"][j] += v[j]
            elif k == "levers":
                for j, n in v.items():
                    t["levers"][j] = t["levers"].get(j, 0) + n
            elif k == "think":
                t["think"] += v
            else:
                t[k] += v
    return t


def ratio(a, b, scale=1.0):
    return scale * a / b if b else float("nan")


def metrics(a):
    """The census metrics of one accumulator (one seed or a block): name -> value."""
    fm = a["foot_s"] / 60
    m = {
        "recycles": a["recycles"],
        "relocations": a["relocations"],
        "stand20 foot-s": a["stand20"],
        "stand20 % of foot": ratio(a["stand20"], a["foot_s"], 100),
        "recover % of foot": ratio(a["recover"], a["foot_s"], 100),
        "B12 cover % of fight": ratio(a["b12"], a["fight"], 100),
        "kills/min": ratio(a["kills"], a["round_ticks"] / TICK_HZ / 60),
        "kills/foot-min": ratio(a["kills"], fm),
        "flips/fight-min": ratio(a["flips"], a["fight"] / 60),
        "d1 bot far %": ratio(a["d1"]["bot_far"], a["d1"]["bot"], 100),
    }
    if a["think"]:
        th = a["think"]
        m["think p50 us/call"] = statistics.median(t[1] for t in th)
        m["think p99 us/call"] = statistics.median(t[2] for t in th)
        m["think p50 us/think"] = statistics.median(t[5] for t in th)
        m["think p99 us/think"] = statistics.median(t[6] for t in th)
    return m


COUNTS = ("recycles", "relocations", "stand20 foot-s")
TIMED = ("think p50 us/call", "think p99 us/call", "think p50 us/think", "think p99 us/think")


def per_seed_mean(seeds, name):
    """A metric as a per-seed value (counts per seed), for the noise band."""
    v = [metrics(a)[name] for a in seeds.values()]
    return [x for x in v if not math.isnan(x)]


def block_value(seeds, name):
    """Counts as the mean per seed; ratios over the whole block."""
    if name in COUNTS:
        return statistics.mean(metrics(a)[name] for a in seeds.values())
    return metrics(merge(seeds.values())).get(name, float("nan"))


def band(values, n_a, n_b):
    """The 95 % band of a block difference: two standard deviations of (mean of n_a seeds) - (mean of n_b seeds),
    the per-seed spread estimated from values."""
    if len(values) < 2 or not n_a or not n_b:
        return float("nan")
    return 2 * statistics.stdev(values) * math.sqrt(1 / n_a + 1 / n_b)


def block_band(seeds, name, n_a, n_b):
    """band() for one metric, the spread taken over seeds ({seed: acc}, base-code blocks only)."""
    return band(per_seed_mean(seeds, name), n_a, n_b)


def fmt(x):
    if isinstance(x, float) and math.isnan(x):
        return "-"
    if isinstance(x, int):
        return str(x)
    return f"{x:.3f}" if abs(x) < 1 else f"{x:.2f}" if abs(x) < 100 else f"{x:.1f}"


def census(a, b, ticks):
    A = load_dir(a, ticks)
    B = load_dir(b, ticks) if b else None
    names = list(metrics(merge(A.values())))
    both = {**A, **(B or {})}
    rows = ["| metric | " + (f"A ({len(A)} seeds) | B ({len(B)} seeds) | both | abs(A - B) | band | noise |" if B
                             else f"A ({len(A)} seeds) | band ({len(A)} against {len(A)}) |"),
            "|---|" + ("---:|" * (6 if B else 2))]
    for n in names:
        va = block_value(A, n)
        if B:
            vb = block_value(B, n)
            d = abs(va - vb)
            s = block_band(both, n, len(A), len(B))
            noise = max(d, s) if not math.isnan(s) else d
            rows.append(f"| {n} | {fmt(va)} | {fmt(vb)} | {fmt(block_value(both, n))} | {fmt(d)} | {fmt(s)} | "
                        f"{fmt(noise)} |")
        else:
            rows.append(f"| {n} | {fmt(va)} | {fmt(block_band(A, n, len(A), len(A)))} |")
    t = merge(both.values())
    rows += ["", f"rounds {t['rounds']} finished {t['finished']}; recycles and relocations and stand20 foot-s are the "
             "mean per seed (all maps); the rest are ratios over the block; think rows are machine time (median over "
             "rounds); band = 2 sd sqrt(1/n_a + 1/n_b), sd the per-seed spread over every block shown", ""]
    rows += lever_table({"all": t})
    return "\n".join(rows)


def lever_table(groups):
    """Levers per foot-minute, one column per group ({name: acc})."""
    names = sorted({k for g in groups.values() for k in g["levers"]})
    out = ["| lever per foot-min | " + " | ".join(groups) + " |", "|---|" + "---:|" * len(groups)]
    for n in names:
        out.append(f"| {n} | " + " | ".join(fmt(ratio(g["levers"].get(n, 0), g["lever_foot_min"]))
                                            for g in groups.values()) + " |")
    out.append("| (foot-min) | " + " | ".join(f"{g['lever_foot_min']:.0f}" for g in groups.values()) + " |")
    return out


def compare(base, cand, noise_dir, ticks):
    B = load_dir(base, ticks)
    C = load_dir(cand, ticks)
    N = load_dir(noise_dir, ticks) if noise_dir else None
    rows = ["| metric | base | cand | cand - base | noise | verdict |", "|---|---:|---:|---:|---:|---|"]
    bad = 0
    for n in metrics(merge(B.values())):
        vb, vc = block_value(B, n), block_value(C, n)
        noise = block_band({**B, **(N or {})}, n, len(B), len(C))
        if N:
            gap = abs(vb - block_value(N, n))
            noise = gap if math.isnan(noise) else max(noise, gap)
        d = vc - vb
        timed = n in TIMED
        ok = math.isnan(d) or abs(d) <= noise
        bad += not ok and not timed
        rows.append(f"| {n} | {fmt(vb)} | {fmt(vc)} | {d:+.3f} | {fmt(noise)} | "
                    f"{'machine time: compare only alternating pinned runs' if timed else 'inside' if ok else 'OUTSIDE'} |")
    return "\n".join(rows), bad


TIMING = re.compile(r"^(\S+) (\d+) wall ([\d.]+) s user ([\d.]+) s")
T975 = (12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160, 2.145, 2.131,
        2.120, 2.110, 2.101, 2.093, 2.086)


def t975(df):
    """Student's t, two-sided 95 %: a handful of timed runs estimates its spread badly, so 2 sd is too narrow."""
    return T975[df - 1] if df <= len(T975) else 1.96 + 2.4 / df


def timing(path, base, cand, budget):
    """The think-cost check: is the candidate's mean user time within `budget` % of the base's? The band is the
    two-block one (pooled run-to-run spread, Student's t for the few runs); a verdict needs the whole band on one side
    of the budget."""
    runs = {}
    for line in read_lines(path):
        if m := TIMING.match(line):
            runs.setdefault(m.group(1), []).append(float(m.group(4)))
    b, c = runs.get(base, []), runs.get(cand, [])
    if len(b) < 2 or len(c) < 2:
        sys.exit(f"bot_census: timing needs two or more runs of {base} and of {cand} in {path}")
    mb, mc = statistics.mean(b), statistics.mean(c)
    pooled = math.sqrt(((len(b) - 1) * statistics.variance(b) + (len(c) - 1) * statistics.variance(c))
                       / (len(b) + len(c) - 2))
    diff = 100 * (mc / mb - 1)
    df = len(b) + len(c) - 2
    bnd = 100 * t975(df) * pooled * math.sqrt(1 / len(b) + 1 / len(c)) / mb
    if diff + bnd <= budget:
        verdict = "within budget"
    elif diff - bnd > budget:
        verdict = "OVER budget"
    else:
        room, rel, need = budget - diff, 100 * pooled / mb, None
        if room > 0:
            need = next((n for n in range(2, 200) if t975(2 * n - 2) * rel * math.sqrt(2 / n) < room), None)
        verdict = "unresolved: " + (f"about {need} runs a side would resolve it" if need else "the difference is at "
                                    "or over the budget; run more pairs")
    lines = [f"{base}: {len(b)} runs, user mean {mb:.2f} s, sd {statistics.stdev(b):.2f} s",
             f"{cand}: {len(c)} runs, user mean {mc:.2f} s, sd {statistics.stdev(c):.2f} s",
             f"{cand} - {base}: {diff:+.2f} %, band +-{bnd:.2f} % (t {t975(df):.2f} x pooled sd x "
             f"sqrt(1/{len(b)} + 1/{len(c)})), "
             f"budget +{budget:g} %: {verdict}"]
    return "\n".join(lines), verdict


def selftest(trials=4000, seed=1):
    """Pure re-rolls of normal per-seed values (sd 1): a 6-seed candidate against a 6-seed base, the spread taken
    over the base and a second base block, as stage 1 does. The band must hold about 95 % of them (the estimated
    spread makes it a little less); the old one-block band (2 sd / sqrt(12)) held about two thirds."""
    import random
    rng = random.Random(seed)
    assert abs(band([0.0, 2.0], 6, 6) - 2 * math.sqrt(2) * math.sqrt(1 / 3)) < 1e-12
    assert math.isnan(band([1.0], 6, 6))
    inside = inside_gap = inside_old = caught = 0
    for _ in range(trials):
        base = [rng.gauss(0, 1) for _ in range(6)]
        other = [rng.gauss(0, 1) for _ in range(6)]
        cand = [rng.gauss(0, 1) for _ in range(6)]
        moved = [x + 3 for x in cand]
        b = band(base + other, 6, 6)
        d = abs(statistics.mean(cand) - statistics.mean(base))
        gap = abs(statistics.mean(base) - statistics.mean(other))
        inside += d <= b
        inside_gap += d <= max(b, gap)
        inside_old += d <= 2 * statistics.stdev(base + other) / math.sqrt(12)
        caught += abs(statistics.mean(moved) - statistics.mean(base)) > max(b, gap)
    cover, cover_gap, cover_old, power = (x / trials for x in (inside, inside_gap, inside_old, caught))
    report = (f"selftest: {trials} simulated re-rolls, 6 seeds against 6: band holds {100 * cover:.1f} %, "
              f"max(band, block gap) {100 * cover_gap:.1f} %, the old 2 SE band {100 * cover_old:.1f} %; "
              f"a move of 3 per-seed sd is caught {100 * power:.1f} %")
    ok = 0.90 <= cover <= 0.97 and cover_gap >= cover and cover_old < 0.75 and power >= 0.9
    return report, ok


def wilson(w, n, z=1.959964):
    if n == 0:
        return (float("nan"), float("nan"))
    p = w / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (100 * (c - h), 100 * (c + h))


def parse_seeds(s):
    if not s:
        return None
    out = set()
    for part in s.split(","):
        lo, _, hi = part.partition("-")
        out.update(range(int(lo), int(hi or lo) + 1))
    return out


def load_dc(path):
    """Every round of a --difficulty-check stderr: dicts with map, seed, tier, side, won, margin and the census lines
    printed in the same block (they come just before the round's own `round` line)."""
    rounds, pending = [], []
    for line in read_lines(path):
        if line.startswith(("d1 ", "levers ", "think ")):
            pending.append(line)
        elif m := ROUND.match(line):
            if m.group(6) is None:
                continue
            side = int(m.group(6))
            winner = None if m.group(4) == "-" else int(m.group(4))
            rounds.append(dict(map=m.group(1), seed=int(m.group(2)), tier=m.group(3), side=side,
                               won=winner == side, finished=winner is not None, margin=int(m.group(5)),
                               lines=pending))
            pending = []
    return rounds


TIERS = ("easy", "normal", "hard", "elite")


def pct1(x):
    """A percentage to one decimal, rounded once from the exact value."""
    return "-" if math.isnan(x) else f"{x:.1f}"


def interval(w, n):
    lo, hi = wilson(w, n)
    return f"{pct1(lo)}-{pct1(hi)}"


def rate(rs):
    w, n = sum(r["won"] for r in rs), len(rs)
    return f"{w}/{n} | {pct1(ratio(w, n, 100))} | {interval(w, n)}"


def dc(path, seeds, maps):
    rs = [r for r in load_dc(path) if (seeds is None or r["seed"] in seeds) and (not maps or r["map"] in maps)]
    if not rs:
        sys.exit(f"bot_census: no proxy rounds in {path}")
    mlist = sorted({r["map"] for r in rs})
    out = [f"rounds {len(rs)}, unfinished {sum(not r['finished'] for r in rs)}, seeds "
           f"{min(r['seed'] for r in rs)}-{max(r['seed'] for r in rs)}, maps {' '.join(mlist)}", "",
           "| tier | all: wins | % | Wilson 95 % | proxy t0: wins | % | Wilson | proxy t1: wins | % | Wilson |",
           "|---|---:|---:|---|---:|---:|---|---:|---:|---|"]
    tiers = [t for t in TIERS if any(r["tier"] == t for r in rs)]
    for t in tiers:
        tr = [r for r in rs if r["tier"] == t]
        out.append(f"| {t} | {rate(tr)} | {rate([r for r in tr if r['side'] == 0])} | "
                   f"{rate([r for r in tr if r['side'] == 1])} |")
    out += ["", "| tier | map | t0 wins | t1 wins | both | % | Wilson 95 % |", "|---|---|---:|---:|---:|---:|---|"]
    for t in tiers:
        for m in mlist:
            mr = [r for r in rs if r["tier"] == t and r["map"] == m]
            w0 = sum(r["won"] for r in mr if r["side"] == 0)
            n0 = sum(r["side"] == 0 for r in mr)
            w1 = sum(r["won"] for r in mr if r["side"] == 1)
            n1 = sum(r["side"] == 1 for r in mr)
            out.append(f"| {t} | {m} | {w0}/{n0} | {w1}/{n1} | {w0 + w1}/{n0 + n1} | "
                       f"{pct1(ratio(w0 + w1, n0 + n1, 100))} | {interval(w0 + w1, n0 + n1)} |")
    # D1 of the bots facing the proxy (non-proxy vs-human groups), beside their own bot targets.
    out += ["", "| tier | human-target thinks | far % | far % of takes | bot-target thinks | far % | far % of takes |",
            "|---|---:|---:|---:|---:|---:|---:|"]
    groups = {}
    for t in tiers:
        facing, allies = blank(), blank()
        for r in (r for r in rs if r["tier"] == t):
            for line in r["lines"]:
                m = D1.match(line) or LEVERS.match(line)
                if not m or m.group(2).startswith("proxy "):
                    continue
                add_census_lines(facing if m.group(2).endswith(" vs-human") else allies, [line])
        d = facing["d1"]
        out.append(f"| {t} | {d['human']} | {fmt(ratio(d['human_far'], d['human'], 100))} | "
                   f"{fmt(ratio(d['human_takes_far'], d['human_takes'], 100))} | {d['bot']} | "
                   f"{fmt(ratio(d['bot_far'], d['bot'], 100))} | {fmt(ratio(d['bot_takes_far'], d['bot_takes'], 100))} |")
        groups[f"{t} facing"] = facing
        groups[f"{t} allies"] = allies
    out += ["", "Levers per foot-minute: `facing` = the bots on the proxy's enemy team (the host row), `allies` = the "
            "proxy's own bots (their DTEAM row).", ""]
    out += lever_table(groups)
    th = [tuple(float(x) for x in m.groups()[1:]) for r in rs for line in r["lines"] if (m := THINK.match(line))]
    if th:
        out += ["", f"think (machine time, median over {len(th)} rounds): per call p50 "
                f"{fmt(statistics.median(t[1] for t in th))} p99 {fmt(statistics.median(t[2] for t in th))} us; per think "
                f"tick p50 {fmt(statistics.median(t[5] for t in th))} p99 {fmt(statistics.median(t[6] for t in th))} us; "
                f"proxy per call p50 {fmt(statistics.median(t[8] for t in th))} p99 "
                f"{fmt(statistics.median(t[9] for t in th))} us"]
    return "\n".join(out)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("census")
    c.add_argument("a")
    c.add_argument("b", nargs="?")
    c.add_argument("--ticks", type=int, default=60000, help="the sweep's tick limit (unfinished rounds' length)")
    p = sub.add_parser("compare")
    p.add_argument("base")
    p.add_argument("cand")
    p.add_argument("--noise")
    p.add_argument("--ticks", type=int, default=60000)
    d = sub.add_parser("dc")
    d.add_argument("err")
    d.add_argument("--seeds")
    d.add_argument("--maps", nargs="*")
    t = sub.add_parser("timing")
    t.add_argument("file")
    t.add_argument("--base", default="main")
    t.add_argument("--cand", default="branch")
    t.add_argument("--budget", type=float, default=5.0)
    sub.add_parser("selftest")
    a = ap.parse_args(argv)
    if a.cmd == "census":
        print(census(a.a, a.b, a.ticks))
    elif a.cmd == "compare":
        text, bad = compare(a.base, a.cand, a.noise, a.ticks)
        print(text)
        return 3 if bad else 0
    elif a.cmd == "timing":
        text, verdict = timing(a.file, a.base, a.cand, a.budget)
        print(text)
        return 3 if verdict.startswith("OVER") else 0
    elif a.cmd == "selftest":
        text, ok = selftest()
        print(text)
        print("selftest " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1
    else:
        print(dc(a.err, parse_seeds(a.seeds), a.maps))
    return 0


if __name__ == "__main__":
    sys.exit(main())
