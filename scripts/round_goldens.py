#!/usr/bin/env python3
"""round_goldens.py - pinned whole-round sim hashes for space and ground.

Each golden is one seeded headless round. Its whole-sim hash (`Sim::hash`) at fixed ticks is compared with a pin.

  space  bf2_spa1 Space Assault and bf2_spa1 one-flag CTF, bots only, Normal, seed 42, 9000 ticks,
         through the game's own battle builder: `game --sim-trace ... --mode ass|1flag --mission ... --no-human
         --state-hashes`. Pinned at ticks 1800, 3600, 5400 and 9000. They need a space bake and space missions,
         which the shared development bake does not hold: `--space-assets` / `--space-missions` (or the
         pins file's [inputs], or OPENBF_SPACE_ASSETS / OPENBF_SPACE_MISSIONS).
  ground bf1_yav2, tat1 and bf1_nab2 (round_to_outcome's CHECK_MAPS), GCW, one full round per tier (easy, normal,
         hard, elite), seed 1, 16 a side on the mission's tickets, a Good human proxy on team 0:
         `round_to_outcome 90000 1 <map>` with SWEEP_DIFFICULTY / SWEEP_PROXY=0:good. Pinned every 3000 ticks
         (SWEEP_HASH_EVERY) and at the round's end. Inputs: the tree's engine/assets_baked and engine/missions
         (`--ground-assets` / `--ground-missions`).

These pin what the sweep and arena gates do not: a human-proxy round at every tier (the human-only rules) and the
space round rules.

Verdicts, one row per golden:
  PASS   every pinned hash matches.
  MOVED  a hash differs and a second run gives the same hashes (a real change; the row names the first tick).
  FAIL   a missing binary, map, mission or pin; a run that failed or printed no hash; a second run that differs
         from the first (nondeterminism); or the inputs' world fingerprint (scripts/world_fingerprint.py) differs
         from the pin's ("inputs changed: re-bless"). Missing assets FAIL; nothing is ever skipped.
Exit: 0 all PASS, 3 some MOVED and none FAIL, 1 any FAIL. `--out DIR` keeps the logs and a results.json.

Usage (from the repository root; build first: cargo build --release -p game --bin game --example round_to_outcome):
  scripts/round_goldens.py                         # check every golden against the pins
  scripts/round_goldens.py --only space            # space goldens only (or ground, or golden names)
  scripts/round_goldens.py --bless > pins.toml     # re-bless: runs each golden twice, PRINTS the pins file

Re-bless: --bless never writes the pins. It refuses (exit 1) unless both runs of every golden agree. Replacing the
pins file needs a card that declares the move and the maintainer's approval; moving a space pin also needs the space
owner's agreement, recorded in the card. After an input change (a rebake makes every ground golden FAIL "inputs
changed"), bless with a binary built from main, then check the branch against those pins: a branch binary would fold
its own behaviour change into the new pins.

Options: --target <cargo target dir> (default $CARGO_TARGET_DIR, else <tree>/engine/target), --tree <repo or worktree root> (default: this
repository), --pins <file> (default <tree>/engine/artifacts/loop/round_goldens.toml), --jobs N (rounds at once,
default 4), --out DIR.
"""
import argparse
import concurrent.futures
import datetime
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
PINS = "engine/artifacts/loop/round_goldens.toml"

SPACE_SEED = 42
SPACE_TICKS = 9000
SPACE_CHECKPOINTS = (1800, 3600, 5400, 9000)
SPACE = (("spa1_ass", "bf2_spa1", "ass"), ("spa1_1flag", "bf2_spa1", "1flag"))

GROUND_SEED = 1
GROUND_MAX_TICKS = 90000
GROUND_EVERY = 3000
GROUND_MAPS = ("bf1_yav2", "tat1", "bf1_nab2")
GROUND_TIERS = ("easy", "normal", "hard", "elite")
GROUND_ENV = {"SWEEP_THREADS": "1", "SWEEP_PROXY": "0:good", "SWEEP_COMBATANTS": "16", "SWEEP_MISSION_TICKETS": "1",
              "SWEEP_HASH_EVERY": str(GROUND_EVERY), "OPENBF_BLACKBOX": "0", "OPENBF_MATCHLOG": "0"}
# Run settings a caller's shell could leak into a round; every golden runs without them.
SCRUB = re.compile(r"^(?:SWEEP_|OPENBF_|ROUND_|STUCK_|HIT_|BOT_|ANIM_|MATCHLOG|ARENA_)")


class Golden:
    def __init__(s, name, kind, game_map, assets, missions, mission, cmd, env):
        s.name, s.kind, s.map = name, kind, game_map
        s.assets, s.missions, s.mission = assets, missions, mission
        s.cmd, s.env = cmd, env


def goldens(a):
    """Every golden, in pin order. `a` carries the binaries and the input folders."""
    out = []
    for name, game_map, mode in SPACE:
        mission = os.path.join(a.space_missions or "", f"{game_map}_{mode}.ron")
        cmd = [a.game, "--sim-trace", "{trace}", str(SPACE_TICKS), "--map", os.path.join(a.space_assets or "", game_map),
               "--mode", mode, "--mission", mission, "--seed", str(SPACE_SEED), "--no-human", "--state-hashes"]
        out.append(Golden(name, "space", game_map, a.space_assets, a.space_missions, mission, cmd, {}))
    for game_map in GROUND_MAPS:
        for tier in GROUND_TIERS:
            env = dict(GROUND_ENV, SWEEP_DIFFICULTY=tier, SWEEP_ASSET_DIR=a.ground_assets,
                       SWEEP_MISSION_DIR=a.ground_missions)
            cmd = [a.rto, str(GROUND_MAX_TICKS), str(GROUND_SEED), game_map]
            mission = os.path.join(a.ground_missions, f"{game_map}_con.ron")
            out.append(Golden(f"{game_map}_{tier}", "ground", game_map, a.ground_assets, a.ground_missions, mission,
                              cmd, env))
    return out


def select(all_goldens, only):
    if not only:
        return all_goldens
    pick = [g for g in all_goldens if g.name in only or g.kind in only]
    unknown = [o for o in only if o not in {g.name for g in all_goldens} | {"space", "ground"}]
    if unknown:
        raise SystemExit(f"round_goldens: unknown golden {unknown} (names: {[g.name for g in all_goldens]})")
    return pick


def missing_inputs(g):
    """The paths a golden needs that are not there."""
    if not g.assets or not g.missions:
        return [f"{g.kind} inputs unset (--{g.kind}-assets / --{g.kind}-missions)"]
    need = [g.cmd[0], g.assets, g.missions, os.path.join(g.assets, g.map, "manifest.ron"), g.mission]
    return [p for p in need if not os.path.exists(p)]


def parse_space(text):
    """{checkpoint tick: hash} from `--state-hashes` output."""
    got = {}
    for m in re.finditer(r"^state (\d+) ([0-9a-f]{16})$", text, re.M):
        if int(m.group(1)) in SPACE_CHECKPOINTS:
            got[m.group(1)] = m.group(2)
    return got, ""


def parse_ground(text, game_map):
    """({checkpoint: hash, "end": hash}, result) from round_to_outcome's stdout for one map."""
    got = {m.group(1): m.group(2)
           for m in re.finditer(rf"^\s+checkpoint {re.escape(game_map)} (\d+) ([0-9a-f]{{16}})$", text, re.M)}
    end = re.search(rf"^{re.escape(game_map)}\s+(team \d wins at tick \d+|NO OUTCOME in \d+ ticks)\b.*\bhash ([0-9a-f]{{16}})$",
                    text, re.M)
    if not end:
        return {}, ""
    got["end"] = end.group(2)
    return got, end.group(1)


def run_once(g, workdir, tree, n):
    """One run: (hashes, result, rc, log path)."""
    log = os.path.join(workdir, f"{g.name}_{n}")
    cmd = [c.replace("{trace}", log + ".trace") for c in g.cmd]
    env = {k: v for k, v in os.environ.items() if not SCRUB.match(k)}
    env.update(g.env)
    with open(log + ".out", "wb") as so, open(log + ".err", "wb") as se:
        rc = subprocess.run(cmd, cwd=os.path.join(tree, "engine"), env=env, stdout=so, stderr=se).returncode
    if os.path.exists(log + ".trace"):
        os.remove(log + ".trace")   # the per-tick trace is large and nothing reads it
    text = open(log + ".out", errors="replace").read()
    hashes, result = parse_space(text) if g.kind == "space" else parse_ground(text, g.map)
    return hashes, result, rc, log


def expected_keys(g):
    if g.kind == "space":
        return [str(t) for t in SPACE_CHECKPOINTS]
    return None   # a ground round ends when it ends: its checkpoints are whatever the pin holds


def complete(g, hashes):
    keys = expected_keys(g)
    return bool(hashes) and (all(k in hashes for k in keys) if keys else "end" in hashes)


def first_moved(pinned, got):
    """The first checkpoint (in tick order, `end` last) whose hash differs, with both values."""
    keys = sorted(set(pinned) | set(got), key=lambda k: (k == "end", int(k) if k.isdigit() else 0))
    for k in keys:
        if pinned.get(k) != got.get(k):
            return k, pinned.get(k), got.get(k)
    return None


def fingerprint(assets, missions, game_map):
    r = subprocess.run([sys.executable, os.path.join(REPO, "scripts/world_fingerprint.py"), assets, missions, game_map],
                       capture_output=True, text=True)
    if r.returncode:
        return None, (r.stderr or r.stdout).strip()
    return json.loads(r.stdout)["sha256"], ""


def judge(g, pin, fp, fp_err, runs, bless):
    """One row: verdict, value text, and the hashes of the first run."""
    hashes, result, rc, log = runs[0]
    row = {"name": g.name, "kind": g.kind, "result": result, "hashes": hashes, "fingerprint": fp, "log": log}
    if rc or not complete(g, hashes):
        return dict(row, verdict="FAIL", value=f"run failed (exit {rc}) or printed no hash: {log}.out / .err")
    if fp is None:
        return dict(row, verdict="FAIL", value=f"world fingerprint failed: {fp_err}")
    if len(runs) > 1 and runs[1][:3] != runs[0][:3]:
        k = first_moved(runs[0][0], runs[1][0])
        return dict(row, verdict="FAIL", value=f"nondeterministic: two runs differ at {k[0] if k else 'exit/result'}")
    if bless:
        return dict(row, verdict="PASS", value="blessed")
    if pin is None:
        return dict(row, verdict="FAIL", value="no pin for this golden (bless)")
    if pin.get("fingerprint") != fp:
        return dict(row, verdict="FAIL",
                    value=f"inputs changed: fingerprint {fp[:12]} vs pinned {str(pin.get('fingerprint'))[:12]} (re-bless)")
    moved = first_moved(pin.get("hashes", {}), hashes)
    if moved is None:
        return dict(row, verdict="PASS", value=f"{len(hashes)} hashes identical ({result or 'space round'})")
    if len(runs) < 2:
        return dict(row, verdict="RERUN", value="")
    k, old, new = moved
    return dict(row, verdict="MOVED", value=f"from tick {k}: {old} -> {new}; now {result or 'space round'}")


def check_one(g, pin, a, workdir, fps):
    miss = missing_inputs(g)
    if miss:
        return {"name": g.name, "kind": g.kind, "verdict": "FAIL", "value": "missing: " + ", ".join(miss)}
    fp, fp_err = fps[(g.assets, g.missions, g.map)].result()
    runs = [run_once(g, workdir, a.tree, 1)]
    if a.bless:
        runs.append(run_once(g, workdir, a.tree, 2))
    row = judge(g, pin, fp, fp_err, runs, a.bless)
    if row["verdict"] == "RERUN":   # differs from the pin: a second run tells a real move from nondeterminism
        runs.append(run_once(g, workdir, a.tree, 2))
        row = judge(g, pin, fp, fp_err, runs, a.bless)
    return row


def toml_str(s):
    return json.dumps(s)


def bless_text(rows, a, head):
    out = [f"# Round goldens: pinned Sim::hash values for scripts/round_goldens.py (re-bless: its header).",
           f"# Blessed {datetime.date.today().isoformat()} at {head} with --target {a.target}.",
           "# After an input change, bless with a binary built from main (the script's header says why).",
           "", "[inputs]", f"space_assets = {toml_str(a.space_assets or '')}",
           f"space_missions = {toml_str(a.space_missions or '')}", ""]
    for r in rows:
        out.append(f"[pins.{r['name']}]")
        if r.get("result"):
            out.append(f"result = {toml_str(r['result'])}")
        out.append(f"fingerprint = {toml_str(r['fingerprint'])}")
        keys = sorted(r["hashes"], key=lambda k: (k == "end", int(k) if k.isdigit() else 0))
        out.append("hashes = { " + ", ".join(f"{toml_str(k)} = {toml_str(r['hashes'][k])}" for k in keys) + " }")
        out.append("")
    return "\n".join(out)


def binaries_used(a, chosen):
    """Each binary the chosen goldens ran, with its build time."""
    out = []
    for path in sorted({g.cmd[0] for g in chosen}):
        try:
            when = datetime.datetime.fromtimestamp(os.path.getmtime(path)).isoformat(sep=" ", timespec="seconds")
        except OSError:
            when = "missing"
        out.append(f"{path} (built {when})")
    return out


def load_pins(path):
    if not os.path.exists(path):
        return None
    with open(path, "rb") as f:
        return tomllib.load(f)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tree", default=REPO)
    ap.add_argument("--target")
    ap.add_argument("--pins")
    ap.add_argument("--space-assets")
    ap.add_argument("--space-missions")
    ap.add_argument("--ground-assets")
    ap.add_argument("--ground-missions")
    ap.add_argument("--only", nargs="+")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--out")
    ap.add_argument("--bless", action="store_true")
    a = ap.parse_args(argv)
    a.tree = os.path.abspath(a.tree)
    a.target = os.path.abspath(a.target or os.environ.get("CARGO_TARGET_DIR") or os.path.join(a.tree, "engine/target"))
    a.game = os.path.join(a.target, "release/game")
    a.rto = os.path.join(a.target, "release/examples/round_to_outcome")
    pins_path = a.pins or os.path.join(a.tree, PINS)
    pins = load_pins(pins_path)
    if pins is None and not a.bless:
        print(f"round_goldens: FAIL no pins file at {pins_path}", file=sys.stderr)
        return 1
    inputs = (pins or {}).get("inputs", {})
    a.space_assets = a.space_assets or os.environ.get("OPENBF_SPACE_ASSETS") or inputs.get("space_assets") or None
    a.space_missions = a.space_missions or os.environ.get("OPENBF_SPACE_MISSIONS") or inputs.get("space_missions") or None
    a.space_assets = a.space_assets and os.path.abspath(a.space_assets)
    a.space_missions = a.space_missions and os.path.abspath(a.space_missions)
    a.ground_assets = os.path.abspath(a.ground_assets or os.path.join(a.tree, "engine/assets_baked"))
    a.ground_missions = os.path.abspath(a.ground_missions or os.path.join(a.tree, "engine/missions"))
    chosen = select(goldens(a), a.only)
    pinned = (pins or {}).get("pins", {})
    workdir = a.out or tempfile.mkdtemp(prefix="round_goldens.")
    os.makedirs(workdir, exist_ok=True)
    with concurrent.futures.ThreadPoolExecutor(max(1, a.jobs)) as pool:
        fps = {}
        for g in chosen:
            key = (g.assets, g.missions, g.map)
            if key not in fps and not missing_inputs(g):
                fps[key] = pool.submit(fingerprint, *key)
        jobs = [pool.submit(check_one, g, pinned.get(g.name), a, workdir, fps) for g in chosen]
        rows = [j.result() for j in jobs]
    for r in rows:
        print(f"[rounds] {r['name']:16s} {r['verdict']:5s} {r['value']}", file=sys.stderr if a.bless else sys.stdout)
    counts = {v: sum(r["verdict"] == v for r in rows) for v in ("PASS", "MOVED", "FAIL")}
    summary = f"round goldens: {counts['PASS']} PASS, {counts['MOVED']} MOVED, {counts['FAIL']} FAIL of {len(rows)}"
    print(summary, file=sys.stderr if a.bless else sys.stdout)
    for b in binaries_used(a, chosen):   # a stale build reads as a misleading PASS or MOVED: name what ran
        print(f"  binary {b}", file=sys.stderr if a.bless else sys.stdout)
    if a.out:
        with open(os.path.join(a.out, "results.json"), "w") as f:
            json.dump(rows, f, indent=1)
    if a.bless:
        if counts["FAIL"]:
            print("round_goldens: not blessed (a golden failed)", file=sys.stderr)
            return 1
        head = subprocess.run(["git", "-C", a.tree, "rev-parse", "--short=8", "HEAD"], capture_output=True,
                              text=True).stdout.strip() or "?"
        print(bless_text(rows, a, head))
        return 0
    return 1 if counts["FAIL"] else (3 if counts["MOVED"] else 0)


if __name__ == "__main__":
    sys.exit(main())
