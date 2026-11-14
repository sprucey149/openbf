#!/usr/bin/env python3
"""Bake one source's gameplay set (missions, maps, sides, classes) into a folder of its own.

    source_gameplay_bake.py --inputs DIR --out OUT --bake BAKE [--extracted DIR]
                            [--groups "tat1 tat2 maps common side classes"] [--bf1-maps ...] [--bf2-maps ...]

Issue #168 (LAN design section 7): players with different copies of the games must
simulate alike. This builds what a player's own pack.py run would build from the
inputs in DIR, minus the sound and loading-screen groups the simulation never
reads, into OUT/assets and OUT/missions, so independent copies can be compared
(`game --static-digest`, `game --net-soak --peer-assets`). It never writes the
shipped engine/assets_baked or engine/missions.

The inputs are chosen by scripts/input_scan.py exactly as pack.py chooses them.
--extracted is where the scan expects extracted trees (PS2 extract-iso output
under bf1/ and bf2/, Xbox extract-xiso output under xbox/); nothing is extracted
here. Lua scripts go to OUT/lua (pack.py's console cache layout).
"""
import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import input_scan  # noqa: E402
import pack  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--inputs", required=True, help="a folder of discs and installs, as pack.py --iso-dir")
    ap.add_argument("--out", required=True)
    ap.add_argument("--bake", required=True, help="a built bake binary")
    ap.add_argument("--extracted", default=os.path.join(ROOT, "extracted"))
    ap.add_argument("--groups", default="tat1 tat2 maps common side classes")
    ap.add_argument("--bf1-maps", default=None, help="rebake.sh OPENBF_BF1_MAPS (e.g. 'TAT/TAT2 NAB/NAB1')")
    ap.add_argument("--bf2-maps", default=None, help="rebake.sh OPENBF_BF2_MAPS")
    ap.add_argument("--skip-missions", action="store_true")
    ap.add_argument("--skip-bake", action="store_true")
    a = ap.parse_args()
    out = Path(a.out).resolve()
    shipped = Path(ROOT, "engine", "assets_baked").resolve()
    if out == shipped or shipped in out.parents or out in shipped.parents:
        sys.exit(f"source_gameplay_bake: {out} overlaps the shipped assets")
    extracted = os.path.abspath(a.extracted)
    plan = input_scan.scan([a.inputs], {"bf1": None, "bf2": None}, {}, extracted, a.inputs)
    print("\n".join(input_scan.summary_lines(plan)), flush=True)
    if plan["refusals"]:
        sys.exit("refused: " + "; ".join(plan["refusals"]))
    pack.EXTRACTED = extracted
    pack.GAMEPLAY_TREES.clear()
    for g, chosen in plan["games"].items():
        if chosen["gameplay"].get("tree"):
            pack.GAMEPLAY_TREES[g] = chosen["gameplay"]["tree"]
        elif chosen["gameplay"]["kind"] == "pc-install":
            pack.GAMEPLAY_TREES[g] = chosen["gameplay"]["path"]
    lua_root = out / "lua"
    missions = out / "missions"
    if not a.skip_missions:
        for g, lvls in pack.LUA.items():
            for sub, lvl in lvls.items():
                d = lua_root / g / sub
                if (d / ".complete").is_file():
                    continue
                d.parent.mkdir(parents=True, exist_ok=True)
                with tempfile.TemporaryDirectory(prefix="lua-", dir=d.parent) as scratch:
                    subprocess.run([sys.executable, "-I", os.path.join(ROOT, "scripts", "extract_lua.py"),
                                    pack.ci_path(pack.lvl_root(g), lvl), "--out", scratch], check=True,
                                   stdout=subprocess.DEVNULL)
                    Path(scratch, ".complete").touch()
                    os.replace(scratch, d)
        missions.mkdir(parents=True, exist_ok=True)
        table_only = [g for g, c in plan["games"].items() if c["gameplay"]["kind"] == "pc-install"]
        env = {k: v for k, v in os.environ.items()
               if k not in ("OPENBF_GAMEPLAY_CORRECTIONS", "OPENBF_GAMEPLAY_SOURCE_BF1", "OPENBF_GAMEPLAY_SOURCE_BF2")}
        command = [sys.executable, "-I", os.path.join(ROOT, "scripts", "gen_missions.py"), "--ps2-only",
                   "--lua-root", str(lua_root), "--out", str(missions), "--hunt", "--space"]
        command += pack.gameplay_correction_args(plan)
        command += [x for g in table_only for x in ("--table-only-game", g)]
        subprocess.run(command, check=True, env=env)
    if a.skip_bake:
        return
    # rebake.sh reads PS2 trees from $ROOT/extracted: a sandbox root whose extracted/ is the one named here.
    sandbox = out / "root"
    sandbox.mkdir(parents=True, exist_ok=True)
    for name, target in (("scripts", os.path.join(ROOT, "scripts")), ("engine", os.path.join(ROOT, "engine")),
                         ("extracted", extracted)):
        link = sandbox / name
        if link.is_symlink():
            link.unlink()
        link.symlink_to(target)
    env = {k: v for k, v in os.environ.items()
           if k not in ("PC1", "PC2", "XB1", "XB2", "BF2_CONSOLE_ISO", "OPENBF_BF1_TREE", "OPENBF_BF2_TREE",
                        "OPENBF_DLC_DIR", "OPENBF_DLC_DONOR", "OPENBF_PC_ONLY", "OPENBF_MENU_FONT_CORE")}
    env.update(input_scan.bake_env(plan))
    env.pop("OPENBF_DLC_DIR", None)
    env.pop("OPENBF_DLC_DONOR", None)
    env.update(OPENBF_INPUTS_CHOSEN="1", OPENBF_PYTHON=sys.executable, OPENBF_CONSOLE_LUA_ROOT=str(lua_root),
               OUT=str(out / "assets"), ROOT=str(sandbox), BAKE=os.path.abspath(a.bake))
    if a.bf1_maps is not None:
        env["OPENBF_BF1_MAPS"] = a.bf1_maps
    if a.bf2_maps is not None:
        env["OPENBF_BF2_MAPS"] = a.bf2_maps
    print("bake env:", {k: env[k] for k in sorted(env) if k.startswith(("OPENBF_", "PC", "XB", "BF2_"))}, flush=True)
    subprocess.run(["bash", os.path.join(ROOT, "scripts", "scratch_bake.sh"), *a.groups.split()], check=True, env=env)


if __name__ == "__main__":
    main()
