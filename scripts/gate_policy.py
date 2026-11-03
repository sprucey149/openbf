#!/usr/bin/env python3
"""Pure gate declaration and verdict helpers, shared by gate self-tests."""

REBAKE_EXPAND = {"maps": ["tat1", "tat2"]}

def parse_declares(s):
    d = {"goldens": set(), "rebake": [], "sweep": False, "arena": False, "sim": False, "leak": False, "parity": []}
    for part in [p.strip() for p in (s or "").split(";") if p.strip()]:
        k, _, v = part.partition("=")
        k = k.strip()
        if k == "goldens":
            d["goldens"] = {x.strip() if x.strip().startswith("engine/goldens/") or x.strip() == "any" else "engine/goldens/" + x.strip()
                            for x in v.split(",") if x.strip()}
        elif k == "rebake":
            for x in v.replace(",", " ").split():
                for g in [x, *REBAKE_EXPAND.get(x, [])]:
                    if g not in d["rebake"]:
                        d["rebake"].append(g)
        elif k == "parity":
            d["parity"] = [x.strip() for x in v.replace(",", " ").split() if x.strip()]
        elif k in ("sweep", "arena", "sim", "leak"):
            d[k] = True
    return d

def landing_gate_groups(groups):
    """Keep every acceptance group selected by the branch's automatic audit."""
    return [g for g in ("ci", "gpu", "parity", "sim", "sweep", "arena", "leak", "bake", "map", "alignment") if g in groups]

def undeclared(rows, dec, moved_goldens):
    bad = []
    for r in rows:
        if r["verdict"] == "ADAPTER LOST":
            bad.append(f"{r['gate']} {r['value']}")
        elif r["verdict"] == "FAIL":
            bad.append(f"{r['gate']} FAILED: {r['value']}")
        elif r["verdict"] == "MOVED":
            g = r["gate"]
            if g == "ci":
                continue
            if g == "gpu":
                if "engine/goldens/any" in dec["goldens"] or "any" in dec["goldens"]:
                    continue
                derived = "engine/goldens/walkabout_stats.ron"  # statistics of the images: declared whenever any image is
                extra = [m for m in moved_goldens if m not in dec["goldens"] and not (m == derived and dec["goldens"])]
                if extra:
                    bad.append(f"undeclared golden moves: {', '.join(extra)}")
            elif g == "bake" and not dec["rebake"]:
                bad.append(f"bake moved but no rebake= declared: {r['value']}")
            elif g == "sweep" and not dec["sweep"]:
                bad.append("sweep moved but `sweep` not declared")
            elif g.startswith("arena") and not dec["arena"]:
                bad.append(f"{g} moved but `arena` not declared")
            elif g == "leak" and not dec["leak"]:
                bad.append("leak moved but `leak` not declared (the collision census, game/examples/leak_census.rs)")
            elif g in ("sim", "simhash") and not dec["sim"]:
                bad.append(f"{g} moved but `sim` not declared: {r['value']}")
            elif g == "parity" and not dec["parity"]:
                bad.append(f"parity moved but no parity= declared: {r['value']}")
    return bad
