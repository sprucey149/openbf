#!/usr/bin/env python3
"""Instructions that turn a PC or Xbox copy's campaign script into the console campaign's.

The console campaign's rules are the one rulebook for Rise of the Empire (PLAN, gameplay decisions). A PC or Xbox copy
ships its own campaign scripts, and they differ in places: an extra timer, a handler wrapped in a test, another icon,
another victory delay, a shield recharge split between the linked objects. `scripts/campaign_ir.py` translates the
copy's own script; a program here then rebuilds the canonical translation out of it.

A program holds no script of its own. It is a tree whose nodes are either a reference to a part of the copy's own
translation (`d`, a path into it), a node rebuilt from such parts (`T` for an expression or statement, `L` for a list),
or a single name or number (`a`). Each program names the SHA-256 of the copy's translation it applies to and of the
result it must give; `apply` refuses anything else, so a program can only ever reproduce the canonical rules from the
copy it was made for.

    python3 scripts/campaign_programs.py build --platform pc --canonical LUA --donor LUA --mission-lvl FILE --out JSON

`build` needs both copies' extracted Lua (`<LUA>/bf2/mission`); the generator only needs the committed table.
"""
import argparse
import hashlib
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

FIELDS = ("name", "params", "vararg", "locals", "body")
SCHEMA = 1
REFERENCE = "ps2-v1"


def canon(x):
    """The translation as JSON values: a tuple node is ["T", ...], a list ["L", ...]."""
    if isinstance(x, tuple):
        return ["T"] + [canon(v) for v in x]
    if isinstance(x, list):
        return ["L"] + [canon(v) for v in x]
    if isinstance(x, dict):
        return ["S", x["main"], ["L"] + [["F"] + [canon(f[k]) for k in FIELDS] for f in x["fns"]]]
    return x


def decanon(x):
    if isinstance(x, list):
        tag, rest = x[0], x[1:]
        if tag == "T":
            return tuple(decanon(v) for v in rest)
        if tag == "L":
            return [decanon(v) for v in rest]
        if tag == "S":
            return {"main": rest[0], "fns": [dict(zip(FIELDS, (decanon(v) for v in f[1:]))) for f in rest[1][1:]]}
        raise ValueError(f"campaign program: unknown node {tag!r}")
    return x


def text(x):
    return json.dumps(x, separators=(",", ":"), sort_keys=True)


def digest(script):
    return hashlib.sha256(text(canon(script)).encode()).hexdigest()


def _index(tree):
    found = {}

    def walk(node, path):
        if isinstance(node, list):
            found.setdefault(text(node), path)
            for i, child in enumerate(node):
                walk(child, path + [i])

    walk(tree, [])
    return found


def make(canonical, donor):
    """The program that rebuilds `canonical` out of `donor` (both translations)."""
    parts = _index(canon(donor))

    def recipe(node):
        if not isinstance(node, list):
            return {"a": node}
        path = parts.get(text(node))
        if path is not None:
            return {"d": path}
        return {node[0]: [recipe(child) for child in node[1:]]}

    return recipe(canon(canonical))


def apply(program, donor, *, donor_sha, result_sha, context="campaign"):
    if digest(donor) != donor_sha:
        raise ValueError(f"{context}: the copy's campaign script is not the one this program was made for")
    tree = canon(donor)

    def build(step):
        (kind, value), = step.items()
        if kind == "a":
            return value
        if kind == "d":
            node = tree
            for i in value:
                node = node[i]
            return node
        if kind in ("T", "L", "S", "F"):
            return [kind] + [build(child) for child in value]
        raise ValueError(f"{context}: unknown program step {kind!r}")

    out = decanon(build(program))
    if digest(out) != result_sha:
        raise ValueError(f"{context}: the campaign program does not reproduce the canonical script")
    return out


def load(path):
    document = json.loads(Path(path).read_bytes())
    if document.get("schema") != SCHEMA or document.get("reference") != REFERENCE:
        raise ValueError(f"unsupported campaign program revision: {path}")
    return document


def canonical_script(table, name, donor, context="campaign"):
    """The canonical translation of campaign script `name` from the copy's own `donor` translation."""
    row = table["scripts"].get(name.lower())
    if row is None:
        raise ValueError(f"{context}: no campaign program record for {name}")
    if row.get("program") is None:
        if digest(donor) != row["result"]:
            raise ValueError(f"{context}: the copy's campaign script is not the canonical one")
        return donor
    return apply(row["program"], donor, donor_sha=row["donor"], result_sha=row["result"], context=context)


# Mission fields read from the copy's own startup code rather than its translation: where a copy still differs
# after its correction record, the table holds the canonical value (`mission`).
SKIP_FIELDS = {"d", "strings", "prov", "pc", "campaign"}


def mission_fields(m):
    fields = {k: canon(v) for k, v in vars(m).items() if k not in SKIP_FIELDS}
    fields["campaign"] = canon({k: v for k, v in m.campaign.items() if k != "script"} | {"main": 0, "fns": []})
    return fields


def loose(x):
    if isinstance(x, list) and x and x[0] in ("T", "L"):
        return [loose(v) for v in x[1:]]
    if isinstance(x, list):
        return [loose(v) for v in x]
    return x


def apply_fields(m, row):
    for key, value in (row.get("mission") or {}).items():
        value = decanon(value)
        if key == "campaign":
            value.pop("main"), value.pop("fns")
            m.campaign.update(value)
        else:
            setattr(m, key, value)


def build_table(platform, canonical_lua, donor_lua, mission_lvl, names, reference):
    import gen_missions as g
    import campaign_ir
    import gameplay_missions

    def translate(root, name):
        g.PS2_BF2 = os.path.join(root, "bf2", "mission")
        f = g.load_ps2("bf2", name)
        if f is None:
            raise ValueError(f"{root}: no {name}")
        return campaign_ir.translate(f)

    scripts = {}
    for name in names:
        want, have = translate(canonical_lua, name), translate(donor_lua, name)
        row = {"donor": digest(have), "result": digest(want)}
        row["program"] = None if row["donor"] == row["result"] else make(want, have)
        if row["program"] is not None:
            apply(row["program"], have, donor_sha=row["donor"], result_sha=row["result"], context=name)
        scripts[name.lower()] = row
    table = {"schema": SCHEMA, "reference": REFERENCE, "game": "bf2", "platform": platform,
             "mission_lvl": hashlib.sha256(Path(mission_lvl).read_bytes()).hexdigest(), "scripts": scripts}
    # The missions as the console copy gives them, then as this copy gives them through its record and programs.
    g.GAMEPLAY_REFERENCES, g.TABLE_ONLY_GAMES = {}, ()
    g.PS2_BF2 = os.path.join(canonical_lua, "bf2", "mission")
    want = {entry[-1]: mission_fields(g.build_campaign(*entry)) for entry in g.CAMPAIGN_MISSIONS}
    g.GAMEPLAY_REFERENCES = gameplay_missions.load([reference], {"bf2": mission_lvl})
    g.TABLE_ONLY_GAMES = ("bf2",) if platform == "pc" else ()
    g.PS2_BF2 = os.path.join(donor_lua, "bf2", "mission")
    g.CAMPAIGN_PROGRAMS[Path(reference).with_name(Path(reference).stem + "-campaign.json")] = table
    for entry in g.CAMPAIGN_MISSIONS:
        have = mission_fields(g.build_campaign(*entry))
        # A tuple where the other build has a list writes the same mission: only a value counts.
        fields = {k: v for k, v in want[entry[-1]].items() if loose(have.get(k)) != loose(v)}
        if set(have) != set(want[entry[-1]]):
            raise ValueError(f"{entry[-1]}: the copy's mission has other fields")
        if fields:
            scripts[entry[-1].lower()]["mission"] = fields
    return table


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="command", required=True)
    b = sub.add_parser("build", help="write a platform's campaign programs")
    b.add_argument("--platform", choices=("pc", "xbox"), required=True)
    b.add_argument("--canonical", required=True, help="the console copy's extracted Lua root")
    b.add_argument("--donor", required=True, help="the platform copy's extracted Lua root")
    b.add_argument("--mission-lvl", required=True, help="the platform copy's MISSION.LVL (its fingerprint)")
    b.add_argument("--reference", required=True, help="the platform's mission correction table")
    b.add_argument("--out", required=True)
    a = ap.parse_args()
    import gen_missions as g
    import campaign_ir
    g.PS2_BF2 = os.path.join(a.canonical, "bf2", "mission")
    scripts = [entry[-1] for entry in g.CAMPAIGN_MISSIONS]
    built = [g.build_campaign(*entry) for entry in g.CAMPAIGN_MISSIONS]
    names = scripts + sorted(g.campaign_libs([m.campaign["script"] for m in built]))
    del campaign_ir
    table = build_table(a.platform, a.canonical, a.donor, a.mission_lvl, names, a.reference)
    Path(a.out).write_text(json.dumps(table, indent=None, separators=(",", ":"), sort_keys=True) + "\n")
    changed = sum(1 for row in table["scripts"].values() if row["program"] is not None)
    print(f"{a.out}: {len(table['scripts'])} scripts, {changed} with a program, {Path(a.out).stat().st_size} bytes")


if __name__ == "__main__":
    main()
