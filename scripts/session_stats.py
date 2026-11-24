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

Schema 2 rounds add, per group: kills and deaths split hero / normal class,
for the humans and for each team's bots (from `class_totals`), and a hero
table: per side, hero class and who played it (human or bot), the lives,
kills, deaths, kills per life, mean life, damage dealt and taken, and how the
lives ended. --hero-lives also lists every hero life. Schema 1 rounds count
in the averages above only.
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
        row["class_split"] = class_split(rs)
        row["heroes"] = hero_table(rs)
        out.append(row)
    return out


SPLIT_FIELDS = ("lives", "kills", "deaths", "team_kills", "damage_dealt", "alive_s")


def who(human, team):
    """`human`, `bot team0`, `bot team1` or `native`."""
    if human:
        return "human"
    return f"bot team{team}" if team in (0, 1) else "native"


def class_split(rounds):
    """Totals of schema-2 `class_totals` per (who, hero or normal), with kills
    and deaths per life."""
    acc = defaultdict(lambda: defaultdict(float))
    for r in rounds:
        for c in r.get("class_totals") or []:
            a = acc[(who(c.get("human"), c.get("team")), "hero" if c.get("hero") else "normal")]
            for f in SPLIT_FIELDS:
                a[f] += c.get(f) or 0
    out = []
    for (w, kind), a in sorted(acc.items()):
        lives = a["lives"] or None
        out.append({"who": w, "kind": kind, **{f: a[f] for f in SPLIT_FIELDS},
                    "kills_per_life": a["kills"] / lives if lives else None,
                    "deaths_per_life": a["deaths"] / lives if lives else None})
    return out


def hero_table(rounds):
    """Schema-2 hero lives per (side, hero class, human or bot)."""
    acc = {}
    for r in rounds:
        for h in (r.get("heroes") or {}).get("lives") or []:
            key = (h.get("side"), h.get("class"), "human" if h.get("human") else "bot")
            a = acc.setdefault(key, {"lives": 0, "kills": 0, "deaths": 0, "life_s": [],
                                     "damage_dealt": 0.0, "damage_taken": 0.0, "ends": defaultdict(int)})
            a["lives"] += 1
            a["kills"] += h.get("kills") or 0
            end = h.get("end", "?")
            a["deaths"] += end not in ("despawned", "removed", "round_end")
            a["ends"][end] += 1
            tick_hz = r.get("tick_hz") or 1
            a["life_s"].append(((h.get("end_tick") or 0) - (h.get("spawn_tick") or 0)) / tick_hz)
            a["damage_dealt"] += h.get("damage_dealt") or 0
            a["damage_taken"] += h.get("damage_taken") or 0
    out = []
    for (side, cls, w), a in sorted(acc.items(), key=lambda kv: tuple(str(k) for k in kv[0])):
        out.append({"side": side, "class": cls, "who": w, "lives": a["lives"], "kills": a["kills"],
                    "deaths": a["deaths"], "kills_per_life": a["kills"] / a["lives"],
                    "mean_life_s": mean(a["life_s"]), "damage_dealt": a["damage_dealt"],
                    "damage_taken": a["damage_taken"], "ends": dict(sorted(a["ends"].items()))})
    return out


def hero_lives(rounds):
    """Every schema-2 hero life, one line each."""
    for r in rounds:
        hz = r.get("tick_hz") or 1
        for h in (r.get("heroes") or {}).get("lives") or []:
            by = h.get("killed_by") or {}
            killer = ""
            if by:
                killer = f" by {'human' if by.get('human') else 'bot'} {by.get('class', '?')}"
                killer += " (hero)" if by.get("hero") else ""
            t = lambda k: fmt(h[k] / hz, 0) if h.get(k) is not None else "-"
            yield (f"{r.get('map', '?')} {r.get('started_utc', '')} {h.get('side')} {h.get('class')} "
                   f"{'human' if h.get('human') else 'bot'} unit {h.get('unit')}: "
                   f"unlocked {t('unlocked_tick')} s offered {t('offered_tick')} s "
                   f"accepted {t('accepted_tick')} s, {t('spawn_tick')}-{t('end_tick')} s, "
                   f"kills {h.get('kills')} dealt {fmt(h.get('damage_dealt'), 0)} "
                   f"taken {fmt(h.get('damage_taken'), 0)}, {h.get('end')}{killer}")


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
        if r.get("class_split"):
            print("  per class kind (schema 2):")
            for c in r["class_split"]:
                print(f"    {c['who']:10} {c['kind']:6} lives {c['lives']:<5.0f} kills {c['kills']:<5.0f} "
                      f"deaths {c['deaths']:<5.0f} k/life {fmt(c['kills_per_life'])}  "
                      f"d/life {fmt(c['deaths_per_life'])}  dealt {fmt(c['damage_dealt'], 0)}")
        if r.get("heroes"):
            print("  heroes:")
            for h in r["heroes"]:
                ends = " ".join(f"{k} {v}" for k, v in h["ends"].items())
                print(f"    {h['side']} {h['class']} {h['who']:5} lives {h['lives']} kills {h['kills']} "
                      f"deaths {h['deaths']} k/life {fmt(h['kills_per_life'])} "
                      f"life {fmt(h['mean_life_s'], 0)} s  dealt {fmt(h['damage_dealt'], 0)} "
                      f"taken {fmt(h['damage_taken'], 0)}  ends: {ends}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("--json", action="store_true", help="print the summary as JSON")
    ap.add_argument("--include-unfinished", action="store_true",
                    help="count rounds left before their outcome")
    ap.add_argument("--hero-lives", action="store_true", help="also list every hero life")
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
        if a.hero_lives:
            for line in hero_lives(rounds):
                print(line)
    for name in bad:
        print(f"skipped unreadable {name}", file=sys.stderr)


if __name__ == "__main__":
    main()
