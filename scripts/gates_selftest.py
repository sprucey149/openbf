#!/usr/bin/env python3
"""gates_selftest.py — checks for the gate shortcuts (no cargo, no GPU, no real repo state):

  * can_move_golden / plan_goldens: which changed files skip the golden step, which run it, the every-N full run;
  * linked_core / skipped_but_linked: the check that the skip list covers no `core` module a golden test links;
  * Gates._pair: one run when it matches the baseline, a second only when it differs (real move vs nondeterminism);
  * bake_group_of / bake_verdict / Gates.g_bake: a diff in the bake code bakes every group and a difference in a group
    the card does not declare is refused with the file named (card BAKEG);
  * adapter_check / run / land.undeclared: a run that would regenerate a golden stops at once with ADAPTER LOST (exit 7)
    when the goldens' GPU is gone; a run that regenerates none, or --adapter-any, is not checked (card LOOPA).

  scripts/gates_selftest.py        # exit 0 = all ok
"""
import os, subprocess, sys, tempfile, tomllib, types

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gates

bad = 0


def check(name, ok, detail=""):
    global bad
    print(f"{'ok  ' if ok else 'FAIL'} {name}{(' — ' + detail) if detail and not ok else ''}")
    bad += 0 if ok else 1


DEPS = tomllib.load(open(os.path.join(HERE, "..", "engine/goldens/deps.toml"), "rb"))

# ---- can_move_golden --------------------------------------------------------------------------------------------
for f, want in [
    ("engine/crates/input/src/lib.rs", False),
    # `core` is not skipped, only its tests and examples: the HUD golden runs a sim (the flown droid's picture)
    ("engine/crates/core/src/ai/steer.rs", True), ("engine/crates/core/src/sim/state.rs", True),
    ("engine/crates/core/src/sim.rs", True), ("engine/crates/core/src/ai.rs", True),
    ("engine/crates/core/src/squad.rs", True),
    ("engine/crates/core/tests/battle_outcome.rs", False), ("engine/crates/core/examples/ttk_trace.rs", False),
    ("engine/crates/core/src/collision/rays.rs", True), ("engine/crates/core/src/collision.rs", True),
    ("engine/crates/core/src/heightfield.rs", True), ("engine/crates/core/src/door.rs", True),
    ("engine/crates/core/src/vehicle/walker.rs", True), ("engine/crates/core/src/fmath.rs", True),
    ("engine/crates/core/src/soldier.rs", True),          # game::animator / game::roster read its enums and constants
    ("engine/crates/core/src/a_new_module.rs", True),     # a new core file is not skipped by default
    ("engine/crates/core/Cargo.toml", True), ("engine/crates/core/src/lib.rs", True),
    ("engine/crates/audio/src/lib.rs", False), ("analysis/re-x.md", False), ("PLAN.md", False),
    ("engine/CRATES.md", False), ("scripts/gates.py", False), ("scripts/cards.py", False),
    ("engine/crates/game/examples/leak_census.rs", False), ("engine/crates/game/tests/roster.rs", False),
    ("engine/goldens/README.md", False),
    ("scripts/goldens.sh", True), ("scripts/rebake.sh", True), ("engine/crates/render/src/lib.rs", True),
    ("engine/crates/game/src/viewer.rs", True), ("engine/crates/game/src/main/headless.rs", True),
    ("engine/crates/ui/src/lib.rs", True), ("engine/crates/bake/src/main.rs", True), ("engine/crates/mungefs/src/lib.rs", True),
    ("engine/crates/game/tests/golden_hud.rs", True), ("engine/crates/game/tests/common/mod.rs", True),
    ("engine/crates/game/tests/walkabout_stats.rs", True), ("engine/goldens/deps.toml", True),
    ("engine/Cargo.lock", True), ("engine/crates/game/Cargo.toml", True), ("engine/goldens/maps/bf1_kas2.png", True),
]:
    check(f"can_move_golden {f} == {want}", gates.can_move_golden(f, DEPS) == want)

# ---- plan_goldens on a throwaway repo ---------------------------------------------------------------------------
tmp = tempfile.mkdtemp()


def git(*a, cwd=None):
    return subprocess.run(["git", "-C", cwd or repo, "-c", "user.email=t@t", "-c", "user.name=t", *a], capture_output=True, text=True, check=True).stdout.strip()


repo = os.path.join(tmp, "repo")
os.makedirs(repo)
git("init", "-q", "-b", "main")
os.makedirs(os.path.join(repo, "engine/goldens"))
open(os.path.join(repo, "engine/goldens/deps.toml"), "w").write(open(os.path.join(HERE, "..", "engine/goldens/deps.toml")).read())


def commit(path, text="x\n", msg="c"):
    full = os.path.join(repo, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    open(full, "a").write(text)
    git("add", "-A")
    git("commit", "-q", "-m", msg)


commit("README.md")
git("branch", "base")
mark = os.path.join(tmp, "mark.sha")
gates.FULL_MARK = mark                       # never the real marker
gates.full_due.__defaults__ = (mark,)        # full_due(tree, every, mark=<this>)
open(mark, "w").write(git("rev-parse", "HEAD") + "\n")   # a full regeneration "just happened"

git("checkout", "-q", "-b", "docs")
commit("analysis/re-new.md"); commit("engine/crates/input/src/lib.rs"); commit("PLAN.md")
g, why = gates.plan_goldens(repo, "base")
check("docs + input only -> no golden group", g == [], f"{g} {why}")

for f in ["engine/crates/core/src/collision/rays.rs", "engine/crates/core/src/heightfield.rs"]:
    git("checkout", "-q", "-b", "core_" + os.path.basename(f), "base")
    commit(f)
    g, why = gates.plan_goldens(repo, "base")
    check(f"only {f} -> every group", g == gates.GOLDEN_GROUPS, f"{g} {why}")
for crate in ["input", "audio"]:
    f = f"engine/crates/{crate}/src/lib.rs"
    git("checkout", "-q", "-b", "skipped_" + crate, "base")
    commit(f)
    g, why = gates.plan_goldens(repo, "base")
    check(f"only {f} -> no golden group", g == [], f"{g} {why}")
git("checkout", "-q", "-b", "grnd", "base")   # card GRND's file set: the old rule skipped all of it
commit("engine/crates/core/src/collision/rays.rs"); commit("engine/crates/core/src/sim/state.rs")
commit("engine/crates/game/examples/ground_probe.rs")
g, why = gates.plan_goldens(repo, "base")
check("collision/rays.rs + sim/state.rs + an example -> every group", g == gates.GOLDEN_GROUPS, f"{g} {why}")

git("checkout", "-q", "-b", "visual", "base")
commit("engine/crates/render/src/lib.rs")
g, why = gates.plan_goldens(repo, "base")
check("a render change -> every group", g == gates.GOLDEN_GROUPS, f"{g} {why}")

git("checkout", "-q", "-b", "mixed", "base")
commit("engine/crates/core/src/sim.rs"); commit("engine/crates/game/src/viewer.rs")
g, why = gates.plan_goldens(repo, "base")
check("sim + a game/src file -> every group", g == gates.GOLDEN_GROUPS, f"{g} {why}")

g, why = gates.plan_goldens(repo, "base", force_all=True)
check("--golden-groups all -> every group", g == gates.GOLDEN_GROUPS)

g, why = gates.plan_goldens(repo, "mixed")
check("HEAD == base (main's own gates) -> every group, never 'none'", g == gates.GOLDEN_GROUPS, f"{g} {why}")

git("checkout", "-q", "docs")
open(mark, "w").write(git("rev-parse", "base") + "\n")
for i in range(10):
    commit(f"engine/crates/core/src/n{i}.rs")
g, why = gates.plan_goldens(repo, "base")
check("10 crate commits since the last full run -> full run due", g == gates.GOLDEN_GROUPS and "due" in why, f"{g} {why}")
open(mark, "w").write("0" * 40 + "\n")
g, why = gates.plan_goldens(repo, "base")
check("a marker that is not an ancestor -> full run due", g == gates.GOLDEN_GROUPS and "due" in why, f"{g} {why}")
os.remove(mark)
check("no marker -> due", gates.full_due(repo, 10, mark))
gates.mark_full(repo, mark)
check("mark_full writes HEAD, then not due", open(mark).read().strip() == git("rev-parse", "HEAD") and not gates.full_due(repo, 10, mark))

# a group may opt out with not_by
git("checkout", "-q", "-b", "optout", "base")
dp = os.path.join(repo, "engine/goldens/deps.toml")
txt = open(dp).read().replace("[groups.hud]\nnot_by = []", '[groups.hud]\nnot_by = ["engine/crates/bake/"]')
open(dp, "w").write(txt)
git("add", "-A"); git("commit", "-q", "-m", "deps")
git("branch", "base2")
commit("engine/crates/bake/src/x.rs")
open(mark, "w").write(git("rev-parse", "base2") + "\n")
g, why = gates.plan_goldens(repo, "base2")
check("a group with not_by drops out when every live file is covered", "hud" not in g and "gallery" in g, f"{g} {why}")
commit("engine/crates/render/src/y.rs")
g, why = gates.plan_goldens(repo, "base2")
check("...and runs again once a file outside its not_by changes", "hud" in g, f"{g} {why}")

# ---- core_link_check: the skip list against what the golden tests link -------------------------------------------
NM = """0000000000aa1000 t <core::collision::world::CollisionWorld>::ray_down
0000000000aa1100 T core::fmath::sin_cos
0000000000aa1200 t <core::heightfield::Heightfield>::height_at
0000000000aa1300 t <core::heightfield::Heightfield>::normal_at::{closure#0}
0000000000aa1400 W <core::door::Door as core::default::Default>::default
0000000000aa1500 t <alloc::vec::Vec<core::sim::Event>>::push
0000000000aa1600 t <core::option::Option<core::ai::Brain>>::is_none
0000000000aa1700 t core::unit::IsUnit::is_unit
0000000000aa1800 t core::fmt::write
0000000000aa1900 d core::sim::SOME_STATIC
0000000000aa1a00 t game::gallery::map_sheet
"""
MODS = ["ai", "collision", "door", "fmath", "heightfield", "sim", "unit"]
got = gates.linked_core(NM, MODS)
check("linked_core: a module's own functions count; std, data and type parameters do not",
      got == {"collision": 1, "fmath": 1, "heightfield": 2, "door": 1}, str(got))
check("skipped_but_linked: today's rule skips nothing the goldens link (collision, heightfield, door, fmath, region, "
      "announcer, vehicle)",
      gates.skipped_but_linked(dict.fromkeys(["collision", "heightfield", "door", "fmath", "region", "announcer", "vehicle"], 1), DEPS) == [])
check("skipped_but_linked: the old whole-crate skip is caught",
      gates.skipped_but_linked(got, {"skip": ["engine/crates/core/"]}) == ["collision", "door", "fmath", "heightfield"])
check("skipped_but_linked: a golden that came to link the sim is caught", gates.skipped_but_linked({"sim": 3, "door": 1}, {"skip": ["engine/crates/core/src/sim"]}) == ["sim"])
check("skipped_but_linked: a skipped FILE of a module directory counts",
      gates.skipped_but_linked({"vehicle": 1}, {"skip": ["engine/crates/core/src/vehicle.rs"]}) == ["vehicle"])
core_src = os.path.join(repo, gates.CORE_SRC)
os.makedirs(os.path.join(core_src, "zdir"), exist_ok=True)
for n in ["lib.rs", "zfile.rs", "zdir.rs"]:
    open(os.path.join(core_src, n), "w").close()
cm = gates.core_modules(repo)
check("core_modules: every core/src/<m>.rs and directory once, without lib",
      "lib" not in cm and cm.count("zdir") == 1 and "zfile" in cm, str(cm))

# ---- Gates._pair --------------------------------------------------------------------------------------------------
bin_dir = os.path.join(tmp, "target/release/examples")
os.makedirs(bin_dir)
counter = os.path.join(tmp, "count")


def prog(name, body):
    p = os.path.join(bin_dir, name)
    open(p, "w").write("#!/bin/bash\n" + body)
    os.chmod(p, 0o755)


base_out = os.path.join(tmp, "baseline.txt")
open(base_out, "w").write("sweep: ok\n")
prog("same", f'echo x >> {counter}; echo "sweep: ok"\n')
prog("moved", f'echo x >> {counter}; echo "sweep: moved"\n')
prog("flaky", f'echo x >> {counter}; echo "sweep: $RANDOM$RANDOM$RANDOM"\n')
prog("broken", f'echo x >> {counter}; exit 3\n')


def pair(cmd):
    out = os.path.join(tmp, "out_" + cmd)
    os.makedirs(out, exist_ok=True)
    if os.path.exists(counter):
        os.remove(counter)
    a = types.SimpleNamespace(tree=repo, target=os.path.join(tmp, "target"), out=out, bake_groups=[], keep_goldens=False)
    G = gates.Gates.__new__(gates.Gates)
    G.tree, G.target, G.out, G.rows = a.tree, a.target, a.out, []
    G.env = dict(os.environ)
    G.build_release = lambda *x: 0
    G._pair("t", cmd, base_out)
    n = len(open(counter).read().split()) if os.path.exists(counter) else 0
    return G.rows[-1]["verdict"], n


v, n = pair("same");   check("pair: output == baseline -> PASS after ONE run", (v, n) == ("PASS", 1), f"{v} {n}")
v, n = pair("moved");  check("pair: differs, deterministic -> MOVED after two runs", (v, n) == ("MOVED", 2), f"{v} {n}")
v, n = pair("flaky");  check("pair: differs and nondeterministic -> FAIL after two runs", (v, n) == ("FAIL", 2), f"{v} {n}")
v, n = pair("broken"); check("pair: nonzero exit -> FAIL after one run", (v, n) == ("FAIL", 1), f"{v} {n}")

import gate_policy as land

# ---- the bake gate: a diff in the bake code bakes every group and refuses an undeclared one (card BAKEG) -----------
for f, want in [
    ("tat1/manifest.ron", "tat1"), ("tat2/collision/a.glb", "tat2"), ("tat1/load/load.ron", "load"),
    ("common_bf2/load/load.ron", "load"), ("common_bf1/common.ron", "common"), ("common_bf2/locl/english.ron", "common"),
    ("bf1_kas2/models/a.glb", "maps"), ("bf2_kam1/load/load.ron", "maps"), ("bf2_kam1/sounds/names.ron", "sounds"),
    ("tat1/sounds/sounds.ron", "sounds"), ("core_bf1/sounds/sounds.ron", "sounds"),
    ("side_bf2/vehicles/rep/collision/rep_fly_vwing.glb", "side"), ("classes/bf1_all.ron", "classes"),
    ("audio/global_bf2/1.ogg", "global-sound"), ("audio/ui_load_lp.ogg", "beep"), ("a_new_dir/x.ron", None),
    ("classes/bf2_dlc.ron", "dlc"), ("side_bf2/dlc_heroes.ron", "dlc"), ("side_bf2/dlc_sounds/a.ogg", "dlc"),
    ("side_bf2/effects/dlc/a.ron", "dlc"),
    # what `dlc_output::install` writes among the retail files; the names sit in a real bake's side_bf2/dlc_heroes.ron
    ("side_bf2/soldiers/rep_inf_kitfisto.glb", "dlc"), ("side_bf2/soldiers/cis_inf_asajjventress.low.skins.ron", "dlc"),
    ("side_bf2/textures/dlch_rep_inf_kitfisto.ktx2", "dlc"), ("side_bf2/weapons/fisto_l_fist.nodes.ron", "dlc"),
    ("side_bf2/weapons/cis_inf_weap_starblade.glb", "dlc"), ("side_bf2/weapons/temp_saber.glb", "dlc"),
    ("side_bf2/weapons/bubblev3.glb", "dlc"),
    ("side_bf2/soldiers/rep_inf_ep3anakin.glb", "side"), ("side_bf2/textures/rep_inf_ep3anakin.ktx2", "side"),
    ("side_bf2/weapons/temp_saber_x.glb", "side"), ("side_bf1/soldiers/rep_inf_kitfisto.glb", "side"),
]:
    check(f"bake_group_of {f} == {want}", gates.bake_group_of(f) == want, str(gates.bake_group_of(f)))
_, _, _, stray_ = gates.bake_verdict("SAME=1 DIFF=1 ONLY_IN_A=0 ONLY_IN_B=0\nDIFF side_bf2/soldiers/rep_inf_kitfisto.glb\n", ["dlc"], True, 2)
check("a dlc card moving a hero's body strays nowhere", stray_ == [], str(stray_))
m_ = __import__("re").search(r'^REBAKE_GROUPS="([^"]*)"', open(os.path.join(HERE, "rebake.sh")).read(), __import__("re").M)
table = {gates.bake_group_of(f) for f in ["tat1/a", "tat2/a", "bf1_x/a", "common_bf1/a", "side_bf1/a", "classes/a",
                                          "x/sounds/a", "audio/global_bf1/a", "tat1/load/a", "audio/a", "classes/bf2_dlc.ron"]}
check("bake_group_of covers exactly rebake.sh's groups", m_ and table == set(m_.group(1).split()), f"{sorted(table)}")

VWING = "side_bf2/vehicles/rep/collision/rep_fly_vwing.glb"
CMP = f"DIFF bf1_kas2/collision/a.glb\nDIFF {VWING}\nA=x\nB=y\nSAME=22000 DIFF=2 ONLY_IN_A=0 ONLY_IN_B=0\n"
MAPS = ["maps", "tat1", "tat2"]  # land.parse_declares("rebake=maps")
v, val, _, stray = gates.bake_verdict(CMP, MAPS, True, 22690)
check("bake_verdict: full bake, a DIFF in undeclared `side` -> FAIL naming the file and the group",
      v == "FAIL" and VWING in val and "rebake=side" in val and stray == [(VWING, "side")], f"{v} {val}")
v, val, _, stray = gates.bake_verdict(CMP, MAPS + ["side"], True, 22690)
check("bake_verdict: full bake, every DIFF in a declared group -> MOVED", (v, stray) == ("MOVED", []), f"{v} {val}")
v, val, _, stray = gates.bake_verdict("A=x\nSAME=22690 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0\n", MAPS, True, 22690)
check("bake_verdict: full bake, nothing differs -> PASS", (v, stray) == ("PASS", []), f"{v} {val}")
v, val, _, stray = gates.bake_verdict(f"ONLY_IN_B {VWING}\nSAME=22689 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=1\n", MAPS, True, 22690)
check("bake_verdict: full bake, an undeclared group's file no longer written -> FAIL", v == "FAIL" and VWING in val, f"{v} {val}")
v, val, _, stray = gates.bake_verdict("DIFF zzz/a.ron\nSAME=9 DIFF=1 ONLY_IN_A=0 ONLY_IN_B=0\n", MAPS, True, 22690)
check("bake_verdict: full bake, a file no group rule knows -> FAIL", v == "FAIL" and "zzz/a.ron" in val, f"{v} {val}")
# unchanged for every other landing: a partial bake and an undeclared full bake read as before
PART = "DIFF bf1_kas2/collision/a.glb\nSAME=5000 DIFF=1 ONLY_IN_A=0 ONLY_IN_B=17000\n"
check("bake_verdict: partial bake, a DIFF -> MOVED (ONLY_IN_B not checked)", gates.bake_verdict(PART, MAPS, False, 22690)[0] == "MOVED")
check("bake_verdict: partial bake, no DIFF -> PASS",
      gates.bake_verdict("SAME=5000 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=17000\n", MAPS, False, 22690)[0] == "PASS")
check("bake_verdict: nothing declared, a DIFF -> MOVED", gates.bake_verdict(CMP, [], False, 22690)[:1] == ("MOVED",))
check("bake_verdict: a bake that wrote nothing -> FAIL",
      gates.bake_verdict("SAME=0 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=22690\n", MAPS, False, 22690)[0] == "FAIL")


def bake_gate(branch, path, declares, cmp_txt):
    """g_bake on a throwaway branch that changes `path`, with the build / scratch bake / cmp faked: (the scratch_bake
    command, the row, land.undeclared's refusals)."""
    git("checkout", "-q", "-b", branch, "main")
    commit(path)
    out = os.path.join(tmp, "out_" + branch)
    cmds = []

    def fake_sh(cmd, cwd, env, out_file, lock=False, gpu=False):
        cmds.append(cmd)
        if "cmp_bake.sh" in cmd:
            open(out_file, "w").write(cmp_txt)
        return 0

    dec = land.parse_declares(declares)
    G = gates.Gates.__new__(gates.Gates)
    G.tree, G.target, G.out, G.rows, G.env, G.base = repo, os.path.join(tmp, "target"), out, [], dict(os.environ), "main"
    G.bake_groups, G.exp = dec["rebake"], {"bake": {"files": 22690}}
    os.makedirs(out, exist_ok=True)
    orig_sh = gates.sh
    gates.sh = fake_sh
    try:
        G.g_bake()
    finally:
        gates.sh = orig_sh
        git("checkout", "-q", "main")
    return next(c for c in cmds if "scratch_bake.sh" in c), G.rows[-1], land.undeclared(G.rows, dec, [])


cmd, row, bad_ = bake_gate("starc", "engine/crates/mungefs/src/coll.rs", "rebake=maps", CMP)
check("bake gate: mungefs/src/coll.rs + rebake=maps bakes EVERY group", cmd.rstrip().endswith("scratch_bake.sh"), cmd)
check("bake gate: ... and the DIFF in `side` is refused with the file named",
      row["verdict"] == "FAIL" and len(bad_) == 1 and VWING in bad_[0] and "rebake=side" in bad_[0], f"{row} {bad_}")
check("bake gate: ... and bake_undeclared.txt lists it",
      open(os.path.join(tmp, "out_starc", "bake_undeclared.txt")).read() == f"side\t{VWING}\n")
cmd, row, bad_ = bake_gate("starc2", "engine/crates/mungefs/src/coll.rs", "rebake=maps,side", CMP)
check("bake gate: the same diff with rebake=maps,side lands (MOVED, declared)", row["verdict"] == "MOVED" and not bad_, f"{row} {bad_}")
cmd, row, bad_ = bake_gate("snd", "engine/crates/game/src/x.rs", "rebake=sounds", PART)
check("bake gate: no bake code in the diff -> the declared groups only, as before",
      cmd.endswith("scratch_bake.sh sounds") and row["verdict"] == "MOVED" and not bad_, f"{cmd} {row}")

# ---- the collision census plan: leak_deps.toml and plan_leak (card GSPD) ------------------------------------------
LDEPS = tomllib.load(open(os.path.join(HERE, "..", "engine/goldens/leak_deps.toml"), "rb"))
for f, want in [
    ("engine/crates/core/src/ai/brain.rs", False), ("engine/crates/core/src/ai.rs", False),
    ("engine/crates/core/src/collision.rs", True), ("engine/crates/core/src/collision/rays.rs", True),
    ("engine/crates/core/src/soldier.rs", True), ("engine/crates/core/src/sim/step.rs", True),
    ("engine/crates/core/src/sim.rs", True), ("engine/crates/core/src/door.rs", True), ("engine/crates/core/src/nav.rs", True),
    ("engine/crates/core/src/a_new_module.rs", True), ("engine/crates/core/src/lib.rs", True),
    ("engine/crates/game/src/walk.rs", True), ("engine/crates/game/src/assets.rs", True), ("engine/crates/game/src/roster.rs", True),
    ("engine/crates/game/src/mission.rs", True), ("engine/crates/game/src/a_new_module.rs", True),
    ("engine/crates/game/examples/leak_census.rs", True), ("engine/crates/game/examples/leak_census_accepted.txt", True),
    ("engine/crates/game/src/animator.rs", False), ("engine/crates/game/src/animator/clips.rs", False),
    ("engine/crates/game/src/main/windowed.rs", False), ("engine/crates/game/src/blackbox.rs", False),
    ("engine/crates/game/src/engine_sound/mix.rs", False), ("engine/crates/game/src/fx/emit.rs", False),
    ("engine/crates/render/src/lib.rs", False), ("engine/crates/ui/src/hud.rs", False), ("engine/crates/input/src/lib.rs", False),
    ("engine/crates/audio/src/lib.rs", False), ("engine/crates/net/src/lib.rs", False),
    ("engine/crates/bake/src/main.rs", True), ("engine/crates/mungefs/src/coll.rs", True), ("engine/crates/manifest/src/lib.rs", True),
    ("engine/missions/bf1_kas2_con.ron", True), ("engine/Cargo.lock", True), ("engine/crates/core/Cargo.toml", True),
    ("analysis/re-x.md", False), ("PLAN.md", False), ("engine/CRATES.md", False), ("scripts/cards.py", False),
    ("scripts/gates.py", True), ("scripts/rebake.sh", True), ("engine/goldens/leak_deps.toml", True),
    ("engine/goldens/deps.toml", False), ("engine/artifacts/loop/STATUS.md", False),
]:
    check(f"can_move_leak {f} == {want}", gates.can_move_leak(f, LDEPS) == want)

open(os.path.join(repo, "engine/goldens/leak_deps.toml"), "w").write(open(os.path.join(HERE, "..", "engine/goldens/leak_deps.toml")).read())
git("checkout", "-q", "main")
git("add", "-A"); git("commit", "-q", "-m", "leak deps", "--allow-empty")
git("branch", "-f", "lbase")
lmark = os.path.join(tmp, "leak_mark.sha")
open(lmark, "w").write(git("rev-parse", "HEAD") + "\n")      # a clean census "just ran"


def lplan(branch_files, name, mark=lmark):
    git("checkout", "-q", "-b", name, "lbase")
    for f in branch_files:
        commit(f)
    return gates.plan_leak(repo, "lbase", mark)


run_it, why = lplan(["engine/crates/core/src/ai/brain.rs", "analysis/re-x.md", "engine/crates/ui/src/hud.rs"], "l_ai")
check("leak plan: ai + docs + ui only -> skipped", run_it is False, why)
run_it, why = lplan(["engine/crates/core/src/ai/brain.rs", "engine/crates/core/src/collision.rs"], "l_col")
check("leak plan: ai + collision.rs -> runs", run_it is True, why)
run_it, why = lplan(["engine/crates/game/src/walk.rs"], "l_walk")
check("leak plan: game/src/walk.rs alone -> runs", run_it is True, why)
run_it, why = lplan(["engine/crates/game/src/animator.rs", "engine/crates/audio/src/lib.rs"], "l_vis")
check("leak plan: animator + audio only -> skipped", run_it is False, why)
run_it, why = lplan(["scripts/gates.py"], "l_gates")
check("leak plan: a change to gates.py itself -> runs", run_it is True, why)
run_it, why = lplan(["engine/crates/core/src/ai/brain.rs"], "l_nomark", mark=os.path.join(tmp, "no_such_mark"))
check("leak plan: no clean census on record -> runs", run_it is True and "due" in why, why)
# the every-N rule: ten crate commits since the clean census
git("checkout", "-q", "-b", "l_ten", "lbase")
for i in range(10):
    commit(f"engine/crates/core/src/ai/m{i}.rs", msg=f"m{i}")
run_it, why = gates.plan_leak(repo, "lbase", lmark)
check("leak plan: 10 crate commits since the last clean census -> runs", run_it is True and "due" in why, why)
git("checkout", "-q", "lbase")
run_it, why = gates.plan_leak(repo, "lbase", lmark)
check("leak plan: HEAD == base -> runs (no diff to judge by)", run_it is True, why)

# ---- Gates.g_leak under the auto plan: skip, run, mark -------------------------------------------------------------
lbase_sha = git("rev-parse", "lbase")
leak_base = os.path.join(tmp, "leak_main_1.txt")
open(leak_base, "w").write("m1  runs 4\nleak: maps 1  samples 1  runs 4  sites 0  accepted 0  new 0\n")
prog("leak_census", f'echo x >> {counter}; cat {leak_base}\n')
prog("leak_census_new", f'echo x >> {counter}; echo "m1  runs 4"; echo "leak: maps 1  samples 1  runs 4  sites 1  accepted 0  new 1"\n')


def leak_gate(branch, plan, prog_name="leak_census"):
    git("checkout", "-q", branch)
    out = os.path.join(tmp, f"out_leak_{branch}_{plan}_{prog_name}")
    os.makedirs(out, exist_ok=True)
    if os.path.exists(counter):
        os.remove(counter)
    lm = os.path.join(tmp, f"lm_{branch}_{plan}_{prog_name}")
    open(lm, "w").write(lbase_sha + "\n")
    gates.LEAK_MARK = lm
    # the program under test is `leak_census`: copy the chosen fake over it
    import shutil
    shutil.copy(os.path.join(bin_dir, prog_name), os.path.join(bin_dir, "leak_census"))
    G = gates.Gates.__new__(gates.Gates)
    G.tree, G.target, G.out, G.rows, G.base, G.leak_plan = repo, os.path.join(tmp, "target"), out, [], "lbase", plan
    G.env = dict(os.environ)
    G.exp = {"baseline_dir": tmp}
    G.build_release = lambda *x: 0
    G.g_leak()
    ran = len(open(counter).read().split()) if os.path.exists(counter) else 0
    marked = open(lm).read().strip() != lbase_sha
    return G.rows, ran, marked


shutil_leak = os.path.join(bin_dir, "leak_census_same")
import shutil
shutil.copy(os.path.join(bin_dir, "leak_census"), shutil_leak)
rows, ran, marked = leak_gate("l_ai", "auto", "leak_census_same")
check("g_leak auto, ai-only diff -> a PASS row 'skipped', the census not run, no mark",
      rows[-1]["verdict"] == "PASS" and "skipped" in rows[-1]["cmd"] and ran == 0 and not marked, f"{rows} {ran} {marked}")
rows, ran, marked = leak_gate("l_col", "auto", "leak_census_same")
check("g_leak auto, collision diff -> runs once, PASS, marks the clean census",
      rows[-1]["verdict"] == "PASS" and ran == 1 and marked, f"{rows} {ran} {marked}")
rows, ran, marked = leak_gate("l_ai", "always", "leak_census_same")
check("g_leak always (the default), ai-only diff -> runs anyway", ran == 1 and rows[-1]["verdict"] == "PASS", f"{rows} {ran}")
rows, ran, marked = leak_gate("l_col", "auto", "leak_census_new")
check("g_leak: a run with `new 1` is not a clean census -> a FAIL row and no mark",
      any(r["verdict"] == "FAIL" for r in rows) and not marked, f"{rows} {marked}")

# build_release (card WEARX): only the examples a gate runs, each linked once in a run; a failed build stops the rest
def build_plan(calls, fail_on=None):
    cmds, real = [], gates.sh
    def fake(cmd, *a, **k):
        cmds.append(cmd)
        return 101 if fail_on and fail_on in cmd else 0
    gates.sh = fake
    G = gates.Gates.__new__(gates.Gates)
    G.tree, G.out, G.rows, G.env = repo, os.path.join(tmp, "out_build"), [], {}
    try:
        rcs = [G.build_release(*c) for c in calls]
    finally:
        gates.sh = real
    return [c.split("-p game ")[1] for c in cmds], rcs, G.rows


cmds, rcs, rows = build_plan([(), ("arena",), ("arena",), ("round_to_outcome",), ("arena", "round_to_outcome")])
check("build_release: the bin, then each example once, never --examples",
      cmds == ["--bins", "--example arena", "--example round_to_outcome"] and rcs == [0] * 5, f"{cmds} {rcs}")
cmds, rcs, rows = build_plan([("leak_census",), ("arena",)], fail_on="leak_census")
check("build_release: a failed build is one FAIL row and no later build",
      cmds == ["--bins --example leak_census"] and rcs == [101, 101] and [r["verdict"] for r in rows] == ["FAIL"], f"{cmds} {rcs} {rows}")

# ---- ADAPTER LOST (card LOOPA) ----------------------------------------------------------------------------------
DEVS = ["Intel(R) Graphics (RPL-S)", "NVIDIA RTX 4000 Ada Generation Laptop GPU", "llvmpipe (LLVM 22.1.8, 256 bits)"]
env0 = os.environ.get("OPENBF_GOLDEN_ADAPTER")
os.environ.pop("OPENBF_GOLDEN_ADAPTER", None)
check("adapter_check: the NVIDIA deviceName present", gates.adapter_check(DEVS)[0])
check("adapter_check: only the iGPU and llvmpipe -> ADAPTER LOST",
      gates.adapter_check([DEVS[0], DEVS[2]]) == (False, gates.adapter_check([DEVS[0], DEVS[2]])[1])
      and gates.adapter_check([DEVS[0], DEVS[2]])[1].startswith("ADAPTER LOST"))
os.environ["OPENBF_GOLDEN_ADAPTER"] = "nosuchgpu"
check("adapter_check: OPENBF_GOLDEN_ADAPTER overrides the name", not gates.adapter_check(DEVS)[0])
os.environ["OPENBF_GOLDEN_ADAPTER"] = "rpl-s"
check("adapter_check: the match is case-insensitive", gates.adapter_check(DEVS)[0])
os.environ.pop("OPENBF_GOLDEN_ADAPTER", None)
check("adapter_check: known empty device list -> ADAPTER LOST",
      not gates.adapter_check([])[0] and gates.adapter_check([])[1].startswith("ADAPTER LOST"))
real_vd = gates.vulkan_devices
gates.vulkan_devices = lambda: None
check("adapter_check: vulkan_devices None -> present, 'not checked'",
      gates.adapter_check() == (True, gates.adapter_check()[1]) and "not checked" in gates.adapter_check()[1])
check("rows_exit: 0 all PASS, 1 a FAIL, 7 ADAPTER LOST",
      [gates.rows_exit(r) for r in ([{"verdict": "PASS"}], [{"verdict": "MOVED"}], [{"verdict": "ADAPTER LOST"}])] == [0, 1, 7])


def adapter_run(devices, plan, any_=False):
    """gates.run with fake groups: (rows, the groups that ran, SUMMARY.md, the state's exit)."""
    ran = []

    class G(gates.Gates):
        def golden_plan(s):
            return plan, "fixture"
    for n in gates.ALL:
        setattr(G, "g_" + n, lambda s, n=n: (ran.append(n), s.row(n, n, 0, "ok", "ok", "PASS")))
    gates.vulkan_devices = lambda: devices
    out = tempfile.mkdtemp(prefix="adapter_", dir=tmp)
    a = types.SimpleNamespace(tree=repo, target=os.path.join(tmp, "t"), out=out, bake_groups=[], keep_goldens=False,
                              golden_groups="auto", base="main", adapter_any=any_)
    rows = gates.run(a, ["ci", "gpu", "map", "protect"], cls=G)
    st = gates.load_state(out)
    return rows, ran, open(os.path.join(out, "SUMMARY.md")).read(), st["exit"], out


rows, ran, summ, ex, out = adapter_run([DEVS[0]], ["tat1"])
check("run: adapter lost + a golden to regenerate -> one ADAPTER LOST row, no group ran, exit 7, SUMMARY's first line",
      [r["verdict"] for r in rows] == ["ADAPTER LOST"] and ran == [] and ex == 7 and summ.startswith("ADAPTER LOST")
      and gates.status_line(out)[1] == 0, f"{rows} {ran} {ex} {summ[:80]!r}")
rows, ran, summ, ex, _ = adapter_run(DEVS, ["tat1"])
check("run: adapter present -> every group runs as before, exit 0", ran == ["ci", "gpu", "map", "protect"] and ex == 0, f"{ran} {ex}")
rows, ran, summ, ex, _ = adapter_run([DEVS[0]], [])
check("run: adapter lost but no golden to regenerate -> not checked, every group runs", ran == ["ci", "gpu", "map", "protect"]
      and ex == 0, f"{ran} {ex}")
rows, ran, summ, ex, _ = adapter_run([DEVS[0]], ["tat1"], any_=True)
check("run: --adapter-any -> not checked", ran == ["ci", "gpu", "map", "protect"] and ex == 0, f"{ran} {ex}")
os.environ["OPENBF_ADAPTER_ANY"] = "1"
rows, ran, summ, ex, _ = adapter_run([DEVS[0]], ["tat1"])
check("run: OPENBF_ADAPTER_ANY=1 -> not checked", ex == 0 and len(ran) == 4, f"{ran} {ex}")
os.environ.pop("OPENBF_ADAPTER_ANY", None)
gates.vulkan_devices = real_vd
if env0 is not None:
    os.environ["OPENBF_GOLDEN_ADAPTER"] = env0
dec = land.parse_declares("goldens=any")
bad_ = land.undeclared([{"gate": "gpu", "verdict": "ADAPTER LOST", "value": "ADAPTER LOST: no Vulkan device"}], dec, [])
check("land.undeclared: an ADAPTER LOST row reads as such (not a move, even with goldens=any)",
      bad_ == ["gpu ADAPTER LOST: no Vulkan device"], f"{bad_}")

check("RE arithmetic fixture referee", subprocess.run(["bash", os.path.join(HERE, "re_calc_selftest.sh")]).returncode == 0)

check("first-divergence trace differ", subprocess.run([sys.executable, os.path.join(HERE, "fdiff.py"), "--self-test"]).returncode == 0)

print("gates_selftest:", "PASS" if not bad else f"{bad} FAILED")
sys.exit(1 if bad else 0)
