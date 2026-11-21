#!/usr/bin/env python3
"""Card DPRX: a player's profile from a MATCH LOG, and the tuning loop that
fits the sweep's human proxy to it.

The windowed game (dev mode, on by default; OPENBF_MATCHLOG=0 turns it off)
writes one match log per battle: engine/artifacts/playtest/matches/
<YYYYmmdd-HHMMSS>_<map>.jsonl when the game runs from engine/
(engine/crates/game/src/matchlog.rs has the format). A headless round
writes the same file for its human proxy (round_to_outcome with
SWEEP_PROXY and MATCHLOG_DIR).

  human_profile.py show LOG [LOG ...] [--json] [--allow-cheats] [--skip-heroes]
      the profile of each log (several: also their sum)
  human_profile.py compare HUMAN_LOG [HUMAN_LOG ...] --proxy LOG [LOG ...]
      [--tol 0.2] [--skip-heroes]
      the human's summed profile against the proxy logs' summed profile,
      per metric, with the gap and OK / OFF against the tolerance
  human_profile.py bins LOG [LOG ...] [--proxy LOG [LOG ...]]
      shot-level hit rate by weapon kind (auto / scoped), his own movement
      (still, move, strafe, air, vehicle) and the range to the enemy
      nearest the ray, plus where his shots go by range: the shape a
      stand-in's accuracy must follow (with --proxy: the two side by side)
  human_profile.py calibrate HUMAN_LOG [HUMAN_LOG ...] [--seeds 4]
      [--first-seed 1] [--grid skill=16,18,20 reaction=6,9] [--strength good]
      [--bin PATH] [--keep DIR] [--tol 0.2] [--fit kd,kills_min,...]
      [--setup FILE] [--mirror-class] [--skip-heroes] [--jobs N]
      the tuning loop: for every point of the grid, play EVERY human log's
      own map, tier, era and team with the proxy on his team (seeds
      first..first+N-1, round_to_outcome with SWEEP_PROXY + MATCHLOG_DIR, 16
      per side on the mission's tickets: SWEEP_COMBATANTS=16
      SWEEP_MISSION_TICKETS=1, both overridable from the environment), sum
      the proxy's logs over all of them, and rank the points by their
      distance to the human's sum on the FIT metrics (or --fit). Prints the
      best point and the `Strength::Good` fields to set in
      engine/crates/core/src/ai/proxy.rs.
      --setup: a JSON object, log file name -> {"allies": N, "foes": N,
      "netplay": 0|1, "combatants": N|"map"}: the round's other humans (more
      proxies of the same point, on his team and on the other), whether it
      was a LAN round (SWEEP_NETPLAY=1, the LAN rows) and its population
      (per side, or the map's defaults; a kill rate depends on it). --mirror-class: the proxy plays the
      class he spent the most time as in that log. --skip-heroes: his hero
      lives are left out (a headless round installs no heroes). --jobs: rounds
      at once. Run the whole loop inside the machine's capped scope
      (scripts/heavy.sh + systemd-run); the rounds are its child processes.

A log with a cheat on at ANY tick (its footer's `cheats`) is refused unless
--allow-cheats (then every line says CHEATS). A log without a footer (the
game crashed before writing one is impossible: it is written whole; but a
hand-cut file) is refused too.

Metrics (one definition for a human and a proxy):
  minutes        from his first spawn to the footer's tick
  kd             kills / deaths (deaths: killed + environmental)
  kills_min      kills per minute
  acc            hits / shots (a hit: an enemy hit, one per victim per tick;
                 a shot: a press - its row's kind only, so schema 2 logs and
                 schema 3's aim columns (direction, zoom, look stick, nearest
                 enemy to the ray) read alike)
  sight_hit_s    median seconds from SEEING an enemy (60 m, 40 deg of his
                 aim, a clear line; again after 5 s unseen) to his first hit
                 on it; sight_hit_n engagements hit, sight_conv their share
  life_s         mean seconds per life
  taken_min      damage taken per minute alive
  dist_min       metres walked per minute on foot (samples 1/6 s)
  strafe_min / strafe_share   sideways over 1 m/s and over the forward speed
  crouch_min / crouch_share, prone_min / prone_share
FIT (the calibration's distance): kd, kills_min, acc, sight_hit_s, life_s.
"""
import argparse
import json
import os
import pathlib
import statistics
import subprocess
import sys
import tempfile

HZ = 30.0
SAMPLE = 5
FIT = ["kd", "kills_min", "acc", "sight_hit_s", "life_s"]
ROOT = pathlib.Path(__file__).resolve().parent.parent
ENGINE = ROOT / "engine"


def load(path, allow_cheats=False):
    lines = pathlib.Path(path).read_text().splitlines()
    if not lines:
        sys.exit(f"{path}: empty")
    head = json.loads(lines[0])
    if head.get("kind") != "header":
        sys.exit(f"{path}: not a match log (first line is not its header)")
    end = json.loads(lines[-1])
    if not isinstance(end, dict) or end.get("kind") != "end":
        sys.exit(f"{path}: no footer: refused (cut file?)")
    rows = [json.loads(l) for l in lines[1:-1]]
    if end.get("cheats"):
        msg = f"{path}: CHEATS WERE ON in this match: {', '.join(end['cheats'])}"
        if not allow_cheats:
            sys.exit(msg + " - refused (a calibration match must be clean; --allow-cheats to read it anyway)")
        print("!" * 72 + "\n" + msg + "\n" + "!" * 72, file=sys.stderr)
    return head, rows, end


def life_starts(spawns, ends):
    """The `spawn` rows that start a life: the first, and each one after a
    death. The log also writes a `spawn` row when a live unit's generation
    changes (a class change at a post), which is the same life going on."""
    out, dead = [], True
    for s in sorted(spawns):
        if dead or any(out[-1] <= e <= s for e in ends):
            out.append(s)
        dead = False
    return out


def hero_lives(head, rows, end):
    """The (start, end) ticks of every stretch spent as a hero class (a
    class name with `_hero_`): from its `spawn` row to his next death (or
    next spawn, or the footer). A headless round installs no heroes, so a
    human's hero time is left out when he is compared with a proxy
    (`--skip-heroes`)."""
    u = head["unit"]
    names = end.get("names", {}).get("classes", [])
    ends = sorted([r[1] for r in rows if r[0] == "death"]
                  + [r[1] for r in rows if r[0] == "kill" and r[3] == u])
    spawns = sorted((r[1], r[2], r[3]) for r in rows if r[0] == "spawn")
    out = []
    for i, (s, c, t) in enumerate(spawns):
        name = names[t][c] if t < len(names) and c < len(names[t]) else ""
        if "_hero_" in name:
            nxt = spawns[i + 1][0] - 1 if i + 1 < len(spawns) else end["tick"]
            out.append((s, min([x for x in ends if x >= s] + [nxt, end["tick"]])))
    return out


def main_class(head, rows, end):
    """The class index the subject spent the most alive time as (hero lives
    left out): what `calibrate --mirror-class` plays the proxy as."""
    u = head["unit"]
    names = end.get("names", {}).get("classes", [])
    ends = sorted([r[1] for r in rows if r[0] == "death"]
                  + [r[1] for r in rows if r[0] == "kill" and r[3] == u])
    spawns = sorted((r[1], r[2], r[3]) for r in rows if r[0] == "spawn")
    time = {}
    for i, (s, c, t) in enumerate(spawns):
        nxt = spawns[i + 1][0] if i + 1 < len(spawns) else end["tick"]
        e = min([x for x in ends if x >= s] + [nxt, end["tick"]])
        name = names[t][c] if t < len(names) and c < len(names[t]) else ""
        if "_hero_" not in name:
            time[c] = time.get(c, 0) + max(e - s, 0)
    return max(time, key=time.get) if time else None


def tally(head, rows, end, skip_heroes=False):
    """The raw counts of one log (`skip_heroes`: without his hero lives)."""
    if skip_heroes:
        hero = hero_lives(head, rows, end)
        if hero:
            inside = lambda t: any(s <= t <= e for s, e in hero)
            rows = [r for r in rows if not inside(r[1])]
            cut = sum(e - s for s, e in hero)
            end = dict(end, tick=end["tick"] - cut)
            # Later rows keep their ticks; only the totals drop the hero
            # time (minutes and lives are what the cut changes).
            rows = [[r[0], r[1] - sum(e - s for s, e in hero if e < r[1])] + list(r[2:]) for r in rows]
            # Each cut ends the life it was in (a `cut` row: a life's end,
            # not a death).
            rows += [["cut", s - sum(e2 - s2 for s2, e2 in hero if e2 < s)] for s, _ in hero]
            rows.sort(key=lambda r: r[1])
    u, team = head["unit"], head["team"]
    t = {"kills": 0, "deaths": 0, "shots": 0, "hits": 0, "taken": 0.0, "engage": [], "dropped": 0,
         "lives": [], "alive_ticks": 0, "foot": 0, "dist": 0.0, "strafe": 0, "strafe_starts": 0,
         "crouch": 0, "crouch_starts": 0, "prone": 0, "prone_starts": 0, "minutes": 0.0,
         "cheats": list(end.get("cheats", []))}
    ends = sorted([r[1] for r in rows if r[0] in ("death", "cut")]
                  + [r[1] for r in rows if r[0] == "kill" and r[3] == u])
    spawns = life_starts([r[1] for r in rows if r[0] == "spawn"], ends)
    first = spawns[0] if spawns else head.get("from", 0)
    t["minutes"] = max(end["tick"] - first, 0) / HZ / 60.0
    for s in spawns:
        e = next((x for x in ends if x >= s), end["tick"])
        t["lives"].append(e - s)
    t["alive_ticks"] = sum(t["lives"])
    seen = {}      # (victim, gen) -> open seen ticks
    hit_keys = set()
    for r in rows:
        k = r[0]
        if k == "shot":
            t["shots"] += 1
        elif k == "kill":
            if r[3] == u:
                t["deaths"] += 1
            elif r[2] == u and r[4] != team:
                t["kills"] += 1
        elif k == "death":
            t["deaths"] += 1
        elif k == "seen":
            seen.setdefault((r[2], r[3]), []).append(r[1])
        elif k == "hit":
            _, tick, shooter, victim, gen, vteam, dmg = r[:7]
            if victim == u:
                t["taken"] += dmg
            elif shooter == u and vteam != team and (victim, tick) not in hit_keys:
                hit_keys.add((victim, tick))
                t["hits"] += 1
                opens = [s for s in seen.get((victim, gen), []) if s <= tick]
                if opens:
                    s = max(opens)
                    t["engage"].append((tick - s) / HZ)
                    seen[(victim, gen)] = [x for x in seen[(victim, gen)] if x > tick]
    t["dropped"] = sum(len(v) for v in seen.values())
    prev = None
    strafing_until = -10
    was_c = was_p = False
    for r in rows:
        if r[0] != "s":
            continue
        _, tick, alive, x, y, z, yaw, stance, side, fwd, lx, ly, seat = r
        if not alive or seat != -1:
            prev = None
            was_c = was_p = False
            continue
        t["foot"] += 1
        if prev and tick - prev[0] <= SAMPLE:
            d = ((x - prev[1]) ** 2 + (z - prev[2]) ** 2) ** 0.5
            if d < 10.0:
                t["dist"] += d
        prev = (tick, x, z)
        if abs(side) > 1.0 and abs(side) > abs(fwd):
            t["strafe"] += 1
            if tick > strafing_until + 2 * SAMPLE:
                t["strafe_starts"] += 1
            strafing_until = tick
        c, p = stance == 1, stance == 2
        t["crouch"] += c
        t["prone"] += p
        t["crouch_starts"] += c and not was_c
        t["prone_starts"] += p and not was_p
        was_c, was_p = c, p
    return t


def add(a, b):
    out = {}
    for k, v in a.items():
        out[k] = v + b[k] if not isinstance(v, list) else v + b[k]
    return out


def profile(t):
    mins = max(t["minutes"], 1e-9)
    foot = max(t["foot"] * SAMPLE / HZ / 60.0, 1e-9)
    lives = max(len(t["lives"]), 1)
    alive_min = max(t["alive_ticks"] / HZ / 60.0, 1e-9)
    n = len(t["engage"])
    return {
        "minutes": t["minutes"], "kills": t["kills"], "deaths": t["deaths"],
        "kd": t["kills"] / max(t["deaths"], 1), "kills_min": t["kills"] / mins,
        "shots": t["shots"], "hits": t["hits"], "acc": t["hits"] / max(t["shots"], 1),
        "sight_hit_s": statistics.median(t["engage"]) if n else float("nan"),
        "sight_hit_n": n, "sight_conv": n / max(n + t["dropped"], 1),
        "lives": len(t["lives"]), "life_s": t["alive_ticks"] / HZ / lives,
        "taken_min": t["taken"] / alive_min, "dist_min": t["dist"] / foot,
        "strafe_min": t["strafe_starts"] / foot, "strafe_share": t["strafe"] / max(t["foot"], 1),
        "crouch_min": t["crouch_starts"] / foot, "crouch_share": t["crouch"] / max(t["foot"], 1),
        "prone_min": t["prone_starts"] / foot, "prone_share": t["prone"] / max(t["foot"], 1),
    }


ROWS = [
    ("minutes", "{:.1f}"), ("kills", "{}"), ("deaths", "{}"), ("kd", "{:.2f}"),
    ("kills_min", "{:.2f}"), ("shots", "{}"), ("hits", "{}"), ("acc", "{:.3f}"),
    ("sight_hit_s", "{:.2f}"), ("sight_hit_n", "{}"), ("sight_conv", "{:.2f}"),
    ("lives", "{}"), ("life_s", "{:.1f}"), ("taken_min", "{:.0f}"), ("dist_min", "{:.1f}"),
    ("strafe_min", "{:.2f}"), ("strafe_share", "{:.3f}"), ("crouch_min", "{:.2f}"),
    ("crouch_share", "{:.3f}"), ("prone_min", "{:.2f}"), ("prone_share", "{:.3f}"),
]


def summed(paths, allow, skip_heroes=False):
    total, heads = None, []
    for p in paths:
        head, rows, end = load(p, allow)
        heads.append((head, end))
        t = tally(head, rows, end, skip_heroes)
        total = t if total is None else add(total, t)
    return total, heads


def describe(p, head, end):
    w = end.get("winner")
    won = "-" if w is None else ("WON" if w == head["team"] else "lost")
    cheat = f"  CHEATS {end['cheats']}" if end.get("cheats") else ""
    return (f"{p}: {head['subject']} #{head['unit']} team {head['team']} ({head['factions'][head['team']]})"
            f"  map {head['map']} {head.get('mission', '')} {head.get('era', '')}  difficulty {head['difficulty']}"
            f"  build {head.get('build_commit', '?')}  round {won} tickets {end.get('tickets')} ({end['reason']}){cheat}")


def show(args):
    total = None
    for p in args.logs:
        head, rows, end = load(p, args.allow_cheats)
        t = tally(head, rows, end, args.skip_heroes)
        total = t if total is None else add(total, t)
        pr = profile(t)
        if args.json:
            print(json.dumps({"file": p, "map": head["map"], "difficulty": head["difficulty"],
                              "subject": head["subject"], "cheats": end.get("cheats", []),
                              "main_class": main_class(head, rows, end), **pr}))
            continue
        print(describe(p, head, end))
        for k, f in ROWS:
            print(f"  {k:13} {f.format(pr[k])}")
    if len(args.logs) > 1 and not args.json:
        pr = profile(total)
        print(f"sum of {len(args.logs)} logs:")
        for k, f in ROWS:
            print(f"  {k:13} {f.format(pr[k])}")


# A shot "hits" when a hit by him on an enemy lands within BIN_WINDOW ticks
# after it: the latest such shot whose nearest enemy was the victim, else
# the latest shot. Melee and blast weapons are left out.
BIN_WINDOW = 20
RANGES = ((0, 10), (10, 25), (25, 50), (50, 100), (100, 1e9))


def weapon_kind(name):
    n = name or ""
    if "sniper" in n:
        return "scoped"
    if any(k in n for k in ("rocket", "launcher", "mortar", "detonator", "grenade", "mine", "detpack", "emp")):
        return "blast"
    if "saber" in n:
        return "melee"
    return "auto"


def own_motion(s):
    if s is None:
        return "?"
    side, fwd, seat, stance = s[8], s[9], s[12], s[7]
    if seat != -1:
        return "vehicle"
    if stance in (4, 5, 6, 7, 8):
        return "air"
    if abs(side) > 2.0:
        return "strafe"
    if (side * side + fwd * fwd) ** 0.5 > 2.0:
        return "move"
    return "still"


def range_bin(r):
    if r is None:
        return None
    for lo, hi in RANGES:
        if r < hi:
            return f"{lo}-{hi if hi < 1e9 else ''}"
    return None


def shot_bins(paths):
    import bisect
    import collections
    tot = collections.defaultdict(lambda: [0, 0])
    for path in paths:
        head, rows, end = load(path, True)
        u, team = head["unit"], head["team"]
        wn = end.get("names", {}).get("weapons", [])
        shots, last = [], None
        for r in rows:
            if r[0] == "s":
                last = r
            elif r[0] == "shot" and len(r) >= 11:
                w = wn[r[2]] if r[2] is not None and r[2] < len(wn) else ""
                shots.append([r[1], weapon_kind(w), r[8], range_bin(r[10]), own_motion(last), 0])
        ts = [x[0] for x in shots]
        for r in rows:
            if r[0] != "hit" or r[2] != u or r[5] == team:
                continue
            tick, victim = r[1], r[3]
            i = bisect.bisect_right(ts, tick) - 1
            pick = next((j for j in range(i, -1, -1) if tick - ts[j] <= BIN_WINDOW and shots[j][2] == victim), None)
            if pick is None and i >= 0 and tick - ts[i] <= BIN_WINDOW:
                pick = i
            if pick is not None:
                shots[pick][5] = 1
        for _, kind, _, rng, mot, hit in shots:
            if kind in ("blast", "melee"):
                continue
            for key in ((kind,), (kind, "range", rng), (kind, "motion", mot), (kind, mot, rng)):
                tot[key][0] += 1
                tot[key][1] += hit
    return tot


def bins(args):
    sets = [("him", shot_bins(args.logs))]
    if args.proxy:
        sets.append(("proxy", shot_bins(args.proxy)))
    keys = sorted({k for _, t in sets for k in t}, key=lambda k: (k[0], len(k), str(k)))
    print(f"{'bin':28}" + "".join(f" {n + ' shots':>11} {n + ' hit':>9} {'share':>6}" for n, _ in sets))
    for k in keys:
        cells = ""
        for _, t in sets:
            n, h = t.get(k, [0, 0])
            whole = t.get((k[0],), [0, 0])[0]
            cells += f" {n:11} {h / n if n else float('nan'):9.3f} {n / max(whole, 1):6.2f}"
        print(f"{'/'.join(str(x) for x in k):28}{cells}")


def gap(h, p):
    if h != h or p != p:
        return float("nan")
    return (p - h) / max(abs(h), 1e-9)


def distance(hp, pp, fit=FIT):
    gs = [abs(gap(hp[k], pp[k])) for k in fit]
    gs = [g for g in gs if g == g]
    return sum(g * g for g in gs) / max(len(gs), 1)


def compare(args):
    ht, _ = summed(args.human, args.allow_cheats, args.skip_heroes)
    pt, _ = summed(args.proxy, True)
    hp, pp = profile(ht), profile(pt)
    print(f"{'metric':13} {'human':>9} {'proxy':>9} {'gap':>7}")
    off = 0
    for k, f in ROWS:
        g = gap(hp[k], pp[k])
        mark = ""
        if k in FIT:
            ok = g == g and abs(g) <= args.tol
            off += 0 if ok else 1
            mark = "OK" if ok else "OFF"
        gs = f"{100 * g:+6.0f}%" if g == g else "      -"
        print(f"{k:13} {f.format(hp[k]):>9} {f.format(pp[k]):>9} {gs} {mark}")
    print(f"fit metrics off by more than {100 * args.tol:.0f} %: {off} of {len(FIT)}")
    return off


def proxy_spec(team, strength, point):
    items = ",".join(f"{k}={v}" for k, v in point.items())
    return f"{team}:{strength}" + (f",{items}" if items else "")


def run_round(bin_path, head, strength, point, seed, out_dir, setup=None, ticks=60000):
    """One headless round of the log's map, tier and era with the proxy on
    the log's team (`point` overrides its profile), the log's other humans
    as more proxies of the same point (`setup`: `allies` on his team,
    `foes` on the other; their class is the strength's own) and, with
    `setup["netplay"]`, the LAN round's rows (`SWEEP_NETPLAY=1`). The caller
    runs the whole loop inside the machine's capped scope (heavy.sh +
    systemd-run), so a round is a plain child process."""
    setup = setup or {}
    others = {k: v for k, v in point.items() if k != "class"}
    team = head["team"]
    spec = ";".join([proxy_spec(team, strength, point)]
                    + [proxy_spec(team, strength, others)] * int(setup.get("allies", 0))
                    + [proxy_spec(1 - team, strength, others)] * int(setup.get("foes", 0)))
    env = dict(os.environ)
    env.update({
        "SWEEP_PROXY": spec,
        "SWEEP_DIFFICULTY": head.get("difficulty") or "normal",
        "SWEEP_THREADS": "1",
        "OPENBF_SIM_THREADS": os.environ.get("OPENBF_SIM_THREADS", "1"),
        # The proxy and the tiers live at 16 per side on the mission's tickets
        # (the sweep's own default is the map's team sizes and tickets).
        "SWEEP_COMBATANTS": os.environ.get("SWEEP_COMBATANTS", "16"),
        "SWEEP_MISSION_TICKETS": os.environ.get("SWEEP_MISSION_TICKETS", "1"),
        "SWEEP_NETPLAY": "1" if setup.get("netplay") else "0",
        "MATCHLOG_DIR": str(out_dir),
    })
    # The round's own population when the setup knows it: a number per
    # side (on the mission's tickets), or "map" for the map's defaults.
    pop = setup.get("combatants")
    if pop == "map":
        env.pop("SWEEP_COMBATANTS", None)
        env.pop("SWEEP_MISSION_TICKETS", None)
    elif pop:
        env["SWEEP_COMBATANTS"] = str(int(pop))
    if head.get("mission", "").endswith("_con_cw.ron"):
        env["SWEEP_ERA"] = "cw"
    else:
        env.pop("SWEEP_ERA", None)
    cmd = ["timeout", "900", str(bin_path), str(ticks), str(seed), head["map"]]
    subprocess.run(cmd, env=env, cwd=ENGINE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
    f = pathlib.Path(out_dir) / f"{head['map']}_s{seed}.jsonl"
    return f if f.exists() else None


def calibrate(args):
    """The tuning loop. Every human log is played at its own map, tier,
    team and era (and, from `--setup`, its other humans and LAN rows;
    `--mirror-class`: as the class he played most), `--seeds` seeds each,
    and the proxy's logs of all of them are summed against the human's
    sum: one fit over every map and tier the logs cover."""
    import concurrent.futures as cf
    setups = json.loads(pathlib.Path(args.setup).read_text()) if args.setup else {}
    logs = []
    for path in args.human:
        head, rows, end = load(path, args.allow_cheats)
        logs.append((path, head, main_class(head, rows, end) if args.mirror_class else None,
                     setups.get(pathlib.Path(path).name, {})))
    ht, _ = summed(args.human, args.allow_cheats, args.skip_heroes)
    hp = profile(ht)
    fit = args.fit.split(",") if args.fit else FIT
    grid = [{}]
    for g in args.grid:
        k, vs = g.split("=", 1)
        grid = [dict(p, **{k: v}) for p in grid for v in vs.split(",")]
    bin_path = pathlib.Path(args.bin) if args.bin else ENGINE / "target/release/examples/round_to_outcome"
    if not bin_path.exists():
        sys.exit(f"{bin_path}: build it first (cargo build --release -p game --example round_to_outcome)")
    keep = pathlib.Path(args.keep) if args.keep else pathlib.Path(tempfile.mkdtemp(prefix="dprx_cal_"))
    maps = sorted({(h["map"], h["difficulty"]) for _, h, _, _ in logs})
    print(f"human ({len(logs)} logs, {hp['minutes']:.0f} min, {len(maps)} map/tier pairs): "
          + "  ".join(f"{k} {hp[k]:.3g}" for k in fit), flush=True)
    ranked = []
    for point in grid:
        d = keep / ("_".join(f"{k}{v}" for k, v in point.items()) or "default")
        jobs = []
        for n, (path, head, cls, setup) in enumerate(logs):
            pt = dict(point)
            if cls is not None and "class" not in pt:
                pt["class"] = cls
            for seed in range(args.first_seed, args.first_seed + args.seeds):
                out = d / f"{n:02d}_{pathlib.Path(path).stem}"
                out.mkdir(parents=True, exist_ok=True)
                jobs.append((head, pt, seed, out, setup))
        with cf.ThreadPoolExecutor(max_workers=args.jobs) as ex:
            files = [f for f in ex.map(lambda j: run_round(bin_path, j[0], args.strength, j[1], j[2], j[3], j[4]),
                                       jobs) if f]
        if not files:
            print(f"  {point}: no log written")
            continue
        pt_, _ = summed(files, True)
        pp = profile(pt_)
        dist = distance(hp, pp, fit)
        ranked.append((dist, point, pp))
        print(f"  {point or 'profile as is'}: {len(files)}/{len(jobs)} rounds, distance {dist:.3f}  "
              + "  ".join(f"{k} {pp[k]:.3g}" for k in fit), flush=True)
    if not ranked:
        sys.exit("no point played")
    ranked.sort(key=lambda r: r[0])
    dist, point, pp = ranked[0]
    gaps = [abs(gap(hp[k], pp[k])) for k in fit if gap(hp[k], pp[k]) == gap(hp[k], pp[k])]
    worst = max(gaps) if gaps else float("nan")
    print(f"best: {point or 'profile as is'}  distance {dist:.3f}, largest fit gap {100 * worst:.0f} %"
          f" ({'within' if worst <= args.tol else 'OUTSIDE'} the {100 * args.tol:.0f} % tolerance)")
    for k in fit:
        g = gap(hp[k], pp[k])
        print(f"  {k:13} human {hp[k]:.3g}  proxy {pp[k]:.3g}  gap {100 * g:+.0f} %")
    print("set these in Strength::Good's row (engine/crates/core/src/ai/proxy.rs), then drop PROVISIONAL:")
    names = {"skill": "skill", "reaction": "reaction_ticks", "view": "view_mult", "circles": "circles",
             "strafe": "react[0]", "roll": "react[1]", "move": "react[2]", "grenade": "grenade",
             "hand": "hand (Some(Hand::DEFAULT) / None)", "hfloor": "hand.floor", "hacquire": "hand.acquire",
             "hsettle": "hand.settle", "hup": "hand.up", "htrack": "hand.track", "hair": "hand.air", "hseat": "hand.seat", "head": "headshots", "hunts": "hunts",
             "class": "class", "mend": "mend", "picks": "picks"}
    for k, v in point.items():
        print(f"  {names.get(k, k)} = {v}")
    print(f"logs kept in {keep}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    a = sub.add_parser("show")
    a.add_argument("logs", nargs="+")
    a.add_argument("--json", action="store_true")
    a.add_argument("--allow-cheats", action="store_true")
    a.add_argument("--skip-heroes", action="store_true")
    a.set_defaults(fn=show)
    a = sub.add_parser("compare")
    a.add_argument("human", nargs="+")
    a.add_argument("--proxy", nargs="+", required=True)
    a.add_argument("--tol", type=float, default=0.2)
    a.add_argument("--allow-cheats", action="store_true")
    a.add_argument("--skip-heroes", action="store_true")
    a.set_defaults(fn=compare)
    a = sub.add_parser("bins")
    a.add_argument("logs", nargs="+")
    a.add_argument("--proxy", nargs="+")
    a.set_defaults(fn=bins)
    a = sub.add_parser("calibrate")
    a.add_argument("human", nargs="+")
    a.add_argument("--seeds", type=int, default=4)
    a.add_argument("--first-seed", type=int, default=1)
    a.add_argument("--grid", nargs="*", default=["skill=16,18,20", "reaction=6,9"])
    a.add_argument("--strength", default="good")
    a.add_argument("--bin")
    a.add_argument("--keep")
    a.add_argument("--tol", type=float, default=0.2)
    a.add_argument("--fit", help="comma-separated metrics for the distance (default: FIT)")
    a.add_argument("--setup", help="JSON: log file name -> {allies, foes, netplay}")
    a.add_argument("--mirror-class", action="store_true")
    a.add_argument("--jobs", type=int, default=1)
    a.add_argument("--allow-cheats", action="store_true")
    a.add_argument("--skip-heroes", action="store_true")
    a.set_defaults(fn=calibrate)
    args = ap.parse_args()
    r = args.fn(args)
    sys.exit(1 if isinstance(r, int) and r > 0 else 0)


if __name__ == "__main__":
    main()
