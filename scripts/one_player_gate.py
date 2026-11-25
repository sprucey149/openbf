#!/usr/bin/env python3
"""One-player gate (issue #194, splitscreen phase 0).

Runs the game's own battle setup headless on three maps with one scripted human
(OPENBF_BENCH_PLAYER=script: spawn screens, walking, turning, firing, jumping, reloading, the
weapon switch, the action key, respawn) and compares the sim hash every 30 ticks with the
committed rows in engine/crates/game/tests/data/one_player_gate.json. A refactor of the player
path (WalkSim, input, the render split) must leave every row unchanged.

    python3 scripts/one_player_gate.py --bin engine/target/release/game [--assets DIR] [--update]

A change that is meant to move the rows (a sim rule change) reruns with --update and says why in
its commit. The report also counts the scripted player's deaths and spawns, and the gate fails
if a scenario recorded with a death no longer has one, so a script that stops reaching a death
cannot pass quietly.
"""
import argparse, json, os, pathlib, subprocess, sys, tempfile

# Re-pins, newest first (what moved and why):
# - public pull request #195 (two-flag CTF, saber hit rework, mods, VR): the LAN soak's static
#   digest only. Lightsaber weapons gained `hand_edges` (the hilt-to-hand segments the reworked
#   saber hits test), a new field of the `weapons` digest group. The three scenarios' sim hashes,
#   deaths and spawns did not move.

ROOT = pathlib.Path(__file__).resolve().parent.parent
GOLDEN = ROOT / "engine/crates/game/tests/data/one_player_gate.json"
# Ground BF1, ground BF2 (with vehicles), space assault.
SCENARIOS = [("bf1_hot1", "con"), ("bf2_uta1", "con"), ("bf2_spa1", "ass")]
SEED, AI, TICKS = 1, 8, 900


def run(args, map_name, mode):
    with tempfile.TemporaryDirectory() as tmp:
        out = pathlib.Path(tmp) / "bench.json"
        cmd = [args.bin, "--map", str(pathlib.Path(args.assets) / map_name), "--mode", mode, "--bench",
               "--bench-ticks", str(TICKS), "--bench-frames", "0", "--ai", str(AI), "--seed", str(SEED)]
        env = dict(os.environ, OPENBF_BENCH_DETAIL=str(out), OPENBF_BENCH_PLAYER="script")
        p = subprocess.run(cmd, env=env, capture_output=True, text=True)
        if p.returncode or not out.exists():
            sys.exit(f"{map_name}: bench failed\n{p.stdout[-800:]}\n{p.stderr[-800:]}")
        r = json.loads(out.read_text())
    return {"initial_hash": r["initial_hash"], "sim_hash": r["sim_hash"], "tick_hashes": r["tick_hashes"],
            "player_deaths": r["player_deaths"], "player_spawns": r["player_spawns"]}


def soak(args):
    """The LAN lockstep soak (`game --net-soak`): four peers on bf1_tat2 in virtual time, every peer's
    sim hash checked at every tick it steps. Pinned: the tick and check counts and the static
    digest; it must report no failure, mismatch or desync."""
    with tempfile.TemporaryDirectory() as tmp:
        out = pathlib.Path(tmp) / "soak.json"
        cmd = [args.bin, "--net-soak", "--assets", args.assets, "--missions", str(pathlib.Path(args.assets).parent / "missions"),
               "--ticks", "1500", "--seed", "1", "--out", str(out)]
        p = subprocess.run(cmd, capture_output=True, text=True)
        if not out.exists():
            sys.exit(f"net-soak failed to run\n{p.stdout[-800:]}\n{p.stderr[-800:]}")
        r = json.loads(out.read_text())
    return {"ok": p.returncode == 0 and not r["failures"] and r["mismatches"] == 0 and r["desync_events"] == 0
            and r["static_digests_equal"], "ticks": r["ticks"], "hash_checks": r["hash_checks"],
            "static_digest": r["static_digest"], "map": r["map"]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=str(ROOT / "engine/target/release/game"))
    ap.add_argument("--assets", default=str(ROOT / "engine/assets_baked"))
    ap.add_argument("--update", action="store_true")
    args = ap.parse_args()
    got = {f"{m}:{mode}": run(args, m, mode) for m, mode in SCENARIOS}
    for k, v in got.items():
        print(f"{k}: sim {v['sim_hash']} deaths {v['player_deaths']} spawns {v['player_spawns']}")
    got["lan_soak"] = soak(args)
    print(f"lan_soak: {got['lan_soak']}")
    if args.update:
        GOLDEN.write_text(json.dumps(got, indent=1, sort_keys=True) + "\n")
        print(f"wrote {GOLDEN.relative_to(ROOT)}")
        return
    want = json.loads(GOLDEN.read_text())
    bad = []
    for k, w in want.items():
        g = got.get(k)
        if g is None:
            bad.append(f"{k}: not run")
            continue
        if k == "lan_soak":
            if not g["ok"]:
                bad.append("lan_soak: the soak reported a failure, mismatch or desync")
            if g != w:
                bad.append(f"lan_soak: differs from the pinned run ({g} vs {w})")
            continue
        if g["tick_hashes"] != w["tick_hashes"] or g["initial_hash"] != w["initial_hash"]:
            first = next((i for i, (a, b) in enumerate(zip(g["tick_hashes"], w["tick_hashes"])) if a != b), None)
            bad.append(f"{k}: hashes differ" + (f", first at tick {w['tick_hashes'][first][0]}" if first is not None else ""))
        if w["player_deaths"] > 0 and g["player_deaths"] == 0:
            bad.append(f"{k}: the script no longer reaches a death")
    if bad:
        sys.exit("one-player gate FAILED:\n  " + "\n  ".join(bad))
    print("one-player gate: every row matches")


if __name__ == "__main__":
    main()
