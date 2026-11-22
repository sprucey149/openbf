#!/usr/bin/env python3
"""session_stats.py -- summarise a folder of session stats rounds (dev only).

    python3 scripts/session_stats.py DIR [--json] [--include-unfinished]

DIR holds the per-round JSON files a `session-stats` build writes
(`openbf_data/session_stats/`, fields in engine/crates/game/src/session_stats.rs).
Groups rounds by map / era / tier and prints, per group: rounds, how many had
humans, the humans' win rate (rounds with humans on one side only), the mean
final margin (humans' side minus the other side, in that round's tickets or
points; bots-only rounds: team 0 minus team 1), team 0's win rate, mean
duration, and the human and bot per-player averages (kills, deaths, score,
captures, accuracy, damage dealt; natives such as Jawas apart from the two
sides' bots). Rounds left before the outcome are skipped
unless --include-unfinished. A LAN round is written by every peer: copies of
one round (same build, map, mode, seed, started within 90 s) count once.
"""

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path

PLAYER_FIELDS = ("kills", "deaths", "score", "captures", "team_kills", "vehicle_kills",
                 "damage_dealt", "damage_taken", "accuracy")


def load(folder):
    rounds, bad = [], []
    for p in sorted(Path(folder).glob("*.json")):
        try:
            rounds.append(json.loads(p.read_text(encoding="utf-8")))
        except (OSError, ValueError):
            bad.append(p.name)
    return rounds, bad


SAME_ROUND_S = 90


def dedupe(rounds):
    """One copy of a LAN round seen by several peers (the host's first): the
    same build, map, mode and seed, started within SAME_ROUND_S of each other."""
    seen, out = defaultdict(list), []
    order = {"host": 0, "single": 1, "headless": 2, "guest": 3}
    for r in sorted(rounds, key=lambda r: order.get(r.get("role"), 9)):
        if r.get("role") in ("host", "guest"):
            key = (r.get("build_id"), r.get("map"), r.get("mode"), r.get("seed"))
            t = r.get("started_unix") or 0
            if any(abs(t - k) <= SAME_ROUND_S for k in seen[key]):
                continue
            seen[key].append(t)
        out.append(r)
    return out


def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None


def margin(r):
    s = r.get("final_score") or [0, 0]
    h = r.get("humans_team")
    if h in (0, 1):
        return s[h] - s[1 - h]
    return s[0] - s[1]


def summarise(rounds):
    groups = defaultdict(list)
    for r in rounds:
        groups[(r.get("map", "?"), r.get("era", "?"), r.get("tier", "?"))].append(r)
    out = []
    for (m, era, tier), rs in sorted(groups.items()):
        with_humans = [r for r in rs if r.get("humans")]
        one_side = [r for r in with_humans if r.get("humans_won") is not None]
        decided = [r for r in rs if r.get("winner_team") in (0, 1, 2)]
        row = {
            "map": m, "era": era, "tier": tier, "rounds": len(rs),
            "with_humans": len(with_humans),
            "human_win_rate": mean([1.0 if r["humans_won"] else 0.0 for r in one_side]),
            "team0_win_rate": mean([1.0 if r["winner_team"] == 0 else 0.0 for r in decided]),
            "mean_margin": mean([margin(r) for r in rs]),
            "score_kinds": sorted({r.get("score_kind", "?") for r in rs}),
            "mean_duration_s": mean([r.get("duration_s") for r in rs]),
        }
        by = defaultdict(int)
        for r in rs:
            by[r.get("decided_by", "?")] += 1
        row["decided_by"] = dict(sorted(by.items()))
        every = [p for r in rs for p in r.get("players", [])]
        kinds = (("human", lambda p: p.get("human")),
                 ("bot", lambda p: not p.get("human") and p.get("team") in (0, 1)),
                 ("native", lambda p: not p.get("human") and p.get("team") not in (0, 1)))
        for kind, pick in kinds:
            ps = [p for p in every if pick(p)]
            row[kind] = {"players": len(ps), **{f: mean([p.get(f) for p in ps]) for f in PLAYER_FIELDS}}
        out.append(row)
    return out


def fmt(x, digits=2):
    if x is None:
        return "-"
    if isinstance(x, float):
        return f"{x:.{digits}f}"
    return str(x)


def print_table(rows):
    for r in rows:
        print(f"{r['map']}  {r['era']}  {r['tier']}: {r['rounds']} rounds "
              f"({r['with_humans']} with humans), score {'/'.join(r['score_kinds'])}, "
              f"decided {r['decided_by']}")
        print(f"  humans won {fmt(r['human_win_rate'])}  team0 won {fmt(r['team0_win_rate'])}  "
              f"margin {fmt(r['mean_margin'], 1)}  duration {fmt(r['mean_duration_s'], 0)} s")
        for kind in ("human", "bot", "native"):
            k = r[kind]
            if not k["players"]:
                continue
            print(f"  {kind:5} n={k['players']:<4} " + "  ".join(
                f"{f} {fmt(k[f])}" for f in PLAYER_FIELDS))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("--json", action="store_true", help="print the summary as JSON")
    ap.add_argument("--include-unfinished", action="store_true",
                    help="count rounds left before their outcome")
    a = ap.parse_args()
    rounds, bad = load(a.dir)
    if not a.include_unfinished:
        rounds = [r for r in rounds if r.get("reason") == "outcome"]
    rounds = dedupe(rounds)
    rows = summarise(rounds)
    if a.json:
        json.dump(rows, sys.stdout, indent=1)
        print()
    else:
        if not rows:
            print(f"no rounds in {a.dir}" + ("" if a.include_unfinished else " (try --include-unfinished)"))
        print_table(rows)
    for name in bad:
        print(f"skipped unreadable {name}", file=sys.stderr)


if __name__ == "__main__":
    main()
