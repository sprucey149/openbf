#!/usr/bin/env python3
"""gates.py — run the project's gates on a tree and compare them with engine/artifacts/loop/gates.toml.

One script instead of a hand-written gates.sh per branch. It runs, records and compares; it never decides whether a
move is acceptable — the card's declared moves and the session's judgement do that (scripts/land.py uses the verdicts).

  scripts/gates.py --out <dir> --auto [--base main]         # pick the groups from the files changed since <base>
  scripts/gates.py --out <dir> --groups ci gpu sim sweep     # explicit groups
  scripts/gates.py --out <dir> --groups bake --bake-groups maps
Env/flags: --tree <repo or worktree root> (default: this repo), --target <cargo target dir> (default: <tree>/engine/target).

Groups:
  ci     ./ci.sh (fmt, clippy -D warnings, build, test) — counts passed/failed/ignored vs [ci]
  gpu    gpu_tests.sh --render, then goldens.sh REGENERATES in the tree: moved goldens are quantified (goldens/QUANT.md),
         sheeted (goldens/pair_*.png) and restored from HEAD unless --keep-goldens
  sim    scripts/sim_gates.sh (walk/aim/600 hashes + motion.csv vs motion_base) and `game --sim-hash 100`
  sweep  round_to_outcome 60000 42 twice: the pair identical, and identical to <baseline_dir>/sweep_main_1.txt
  arena  arena 3000 42 flat all / arena 9000 42 tat2 all, twice each: pairs identical and identical to the baselines
  leak   leak_census (card LEAK) twice: the pair identical, and identical to <baseline_dir>/leak_main_1.txt; its footer's
         `new` must be 0 (a leak site no line of game/examples/leak_census_accepted.txt covers)
  bake   scratch_bake.sh [--bake-groups] into <out>/bake, then cmp_bake.sh: DIFF / ONLY_IN_A must be 0 unless declared;
         a bake that wrote no file FAILS (a partial bake's ONLY_IN_B is not checked, so an empty one would pass).
         --bake-groups (the card's declared groups) on a diff that touches the bake code bakes EVERY group and FAILS on
         a difference in a group that is not declared, naming the file and the group (card BAKEG: a crate shared by
         several groups left `side` stale behind a `rebake=maps`)
  alignment  continuous placed-art/corrected-collision certificates against independent PS2 bakes; requires
         --alignment-reference ASSETS and --alignment-case NAME=ASSETS (repeat for each source). Reports every
         base/layer placement, bounds and surface residuals; missing inputs/offenders FAIL. This is an explicit
         developer gate, not part of a normal player build.
  map    scripts/size_report.sh --check-map
  parity scripts/parity.py gate (card PARITY): the port re-rendered at every stored PCSX2 pose (~950 pairs, 32 maps), each
         map's mean 16x16 block error vs main HEAD's (cached per sha in loop/parity/main/); a map past +0.5 FAILS unless
         --parity-allow names it (the card's `declares: parity=<maps>`; land.py passes it) — an allowed rise is MOVED
  fidelity  (card FIDL, 2026-10-08) the bot/map FIDELITY numbers as tracked values with an allowed direction: runs
         `sweep_seeds.py --botq` (toml [fidelity] seeds/ticks; ~4 min) and compares, per map and over all maps, stuck_max,
         recycles, walking in place, stalls, finishes, standing still, a teammate within 3 m, contact, reversals, ... with the
         baselines in gates.toml [fidelity.metrics.*] (each: direction down|up|track, tolerances, source + date). A metric that
         moves the WRONG way past its tolerance FAILS unless the card declares it: `declares: fidelity=<metric[@map]|@map|all>`
         (a regression the card names = MOVED). Better-than-baseline is PASS and listed (re-baseline to lock it in:
         `gates.py --rebaseline-fidelity <out>/fidelity/fidelity_new.json [--only m1,m2]`). `--fidelity-dir <dir>` reuses a
         finished sweep_seeds run instead of running one; `--fidelity-allow` overrides the card's declaration.
         A PER-MAP failure is re-run on [fidelity] confirm_seeds (fresh seeds, 2026-10-09) and kept only if that map is also
         worse there; so is an all-maps failure of a SUM (recycled, noprog, loop20: card FIDA, 2026-10-16 - an 8-seed sum's
         block-to-block spread is about its tolerance), against main's own confirm-block sum. An all-maps MEAN is never
         re-run (it is the sharp test). `--fidelity-confirm-dir <dir>` gives a finished confirm-seeds run to a
         `--fidelity-dir` replay.
         Under --auto it runs with the sweep group (core|game|input|net changes), as extra rows of the same gates run.
  protect   (card FIDL) `git diff <base>...HEAD` over scripts/oracle_guard.py's protected paths (goldens, PS2 references, parity
         cache, gates.toml + loop_baselines, bot_quality_ps2.json, the gate/hook code): FAIL if a card changed one without
         declaring it (`declares: oracle=<class|any>` or `goldens=` for goldens); declared = MOVED. Always part of --auto.
  (land.py passes its own fixed group list, so both new groups also ride on existing ones: protect on `map`, fidelity on
  `sweep`; each runs once per gates run. Run alone with --groups protect / fidelity.)
--auto: always ci, gpu, map, protect; core|game|input|net crates → sim, sweep, arena, leak; render|game crates → parity;
  (leak under --auto, and under --leak-plan auto, runs only when a changed file can move the census — engine/goldens/
  leak_deps.toml — or every 10th crate commit; marker loop/leak_full.sha. A skipped census is a PASS row saying why.)
mungefs|bake|manifest or scripts/{rebake,scratch_bake}.sh → bake. land.py adds parity for a card that declares goldens=.
Shortcuts (2026-09-30; measured in the gate runs since 09-27: leak pair ~7.7 min, goldens ~2 min per run):
  * gpu regenerates only the golden groups a change can move — engine/goldens/deps.toml lists what cannot (docs, the
    input/audio/net crates, core's sim and AI, non-golden tests…) and a diff of only those skips the render tests and
    every golden. A golden run then reads which `core` modules the golden tests link (core_link_check) and FAILS if
    deps.toml skips one of them, so the skip list cannot fall behind the code (card GSKIP).
    Every `full_every` (10) crate commits all groups run again (marker loop/goldens_full.sha). --golden-groups
    auto|all|none|<a,b> overrides; land.py passes the branch run's plan to the main run, which has no diff to judge by.
  * sweep / arena / leak run ONCE: byte-identical to the baseline = PASS; anything else runs a second time to tell a real
    move (pair identical) from nondeterminism (pair differs = FAIL), exactly as the always-twice gate did.
Heavy groups (ci, gpu, bake) run under flock engine/artifacts/loop/ci.lock, like every other gate on this machine; the gpu
group also holds engine/artifacts/loop/gpu.lock (card LANE3: a HW PCSX2 session holds it; a software one does not), always
taken AFTER ci.lock so the two locks have one order.
Writes <out>/SUMMARY.md (a Gates table to paste into a report) and <out>/results.json. Exit 0 = every gate PASS, 1 = a gate
did not, 7 = ADAPTER LOST (the goldens' GPU is gone; nothing ran - see below).

Detached runs (card LDET) — a session never has to die waiting on a gate:
  scripts/gates.py --out <dir> ... --detach      # starts itself in a new session (setsid), log <out>/gates.log; returns
  scripts/gates.py --out <dir> --status          # one line; exit 0 finished, 3 running, 4 dead (resume it), 5 no state
  scripts/gates.py --out <dir> --resume          # re-run only the groups a dead run did not finish (arguments from state)
<out>/gates_state.json is written at every group's start and end: the arguments (tree HEAD and gates.toml hash
included), the finished groups and their rows, the running group, its child pid. A resume keeps a finished group only
when the arguments are unchanged; an unfinished group re-runs from its start.

ADAPTER LOST (card LOOPA, 2026-10-16): the goldens were rendered on the NVIDIA GPU (render/device.rs asks wgpu for
HighPerformance). When it has fallen off the bus, wgpu takes the Intel iGPU and every regenerated golden moves (2026-10-15:
89 per branch, 56 of them out of the diff's reach). So before any group runs, a run whose gpu group would regenerate a
golden asks `vulkaninfo --summary` for a deviceName holding GOLDEN_ADAPTER (env OPENBF_GOLDEN_ADAPTER overrides); absent =
one row `gpu ADAPTER LOST`, no group runs, SUMMARY.md's first line says so, exit 7 (1 stays a gate FAIL). A diff that moves
no golden is not checked; vulkaninfo missing or failing = cannot tell = not checked. --adapter-any / OPENBF_ADAPTER_ANY=1
skips the check.
"""
import argparse, datetime, glob, hashlib, json, math, os, re, shlex, subprocess, sys, tomllib

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
GATES_TOML = os.path.join(REPO, "engine/artifacts/loop/gates.toml")
LOCK = os.path.join(REPO, "engine/artifacts/loop/ci.lock")
GPU_LOCK = os.path.join(REPO, "engine/artifacts/loop/gpu.lock")
ALL = ["ci", "gpu", "parity", "sim", "sweep", "arena", "leak", "bake", "map", "protect", "fidelity", "alignment"]


GOLDEN_ADAPTER = "NVIDIA"  # a substring of the goldens' adapter's vulkaninfo deviceName
ADAPTER_LOST = "ADAPTER LOST"  # the row's verdict; exit 7
EXIT_ADAPTER_LOST = 7


def vulkan_devices():
    """The deviceName lines of `vulkaninfo --summary`, or None when it cannot tell (not installed, failed, hung, no line)."""
    try:
        r = subprocess.run(["vulkaninfo", "--summary"], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        return None
    names = re.findall(r"^\s*deviceName\s*=\s*(.+?)\s*$", r.stdout, re.M)
    return names if r.returncode == 0 and names else None


def adapter_check(devices=None):
    """(present, line): is the goldens' GPU among the Vulkan devices? `devices` None = ask vulkaninfo."""
    want = os.environ.get("OPENBF_GOLDEN_ADAPTER") or GOLDEN_ADAPTER
    devs = vulkan_devices() if devices is None else devices
    if devs is None:
        return True, f"adapter not checked: vulkaninfo cannot tell (want {want!r})"
    hit = [d for d in devs if want.lower() in d.lower()]
    if hit:
        return True, f"adapter {hit[0]!r} present"
    return False, (f"{ADAPTER_LOST}: no Vulkan device matches {want!r} (the GPU the goldens were rendered on); devices: "
                   f"{', '.join(devs)}. Every regenerated golden would move. Fix the GPU (reboot), or --adapter-any / "
                   f"OPENBF_ADAPTER_ANY=1 to render on what is there")


def adapter_any(a=None):
    return bool(getattr(a, "adapter_any", False)) or os.environ.get("OPENBF_ADAPTER_ANY") == "1"


def rows_exit(rows):
    """0 every gate PASS, 7 the goldens' adapter is gone (nothing was judged), 1 a gate did not pass."""
    if any(r["verdict"] == ADAPTER_LOST for r in rows):
        return EXIT_ADAPTER_LOST
    return 0 if all(r["verdict"] == "PASS" for r in rows) else 1


ON_CHILD = None  # run(): records each step's child pid in the state file


def spawn(cmd, cwd, env, stdout, stderr):
    p = subprocess.Popen(cmd, shell=True, cwd=cwd, env=env, stdout=stdout, stderr=stderr)
    if ON_CHILD:
        ON_CHILD(p.pid)
    return p.wait()


def sh(cmd, cwd, env, out_file, lock=False, gpu=False):
    """Run a shell command, stdout+stderr to out_file; return exit code. lock: ci.lock; gpu: ci.lock, then gpu.lock (shared)."""
    if gpu:
        # gpu.lock is taken SHARED, as HW PCSX2 sessions take it (2026-10-11): measured, 3 HW PCSX2 beside the render tests +
        # the golden check change no verdict (18 of 18 suites, 73-76 s against 63-65 alone) and PCSX2 keeps its speed
        # (99.9 % against 100.1 %). Exclusive, a gate waited for every capture to end - and for ever on its own lane's.
        # Two gate runs still take turns: ci.lock.
        cmd = f"flock {LOCK} flock -s {GPU_LOCK} bash -c {json.dumps(cmd)}"
    elif lock:
        cmd = f"flock {LOCK} bash -c {json.dumps(cmd)}"
    with open(out_file, "w") as f:
        return spawn(cmd, cwd, env, f, subprocess.STDOUT)


def run_split(cmd, cwd, env, out, err):
    with open(out, "w") as o, open(err, "w") as e:
        return spawn(cmd, cwd, env, o, e)


def same(a, b):
    return os.path.exists(a) and os.path.exists(b) and open(a, "rb").read() == open(b, "rb").read()


class Gates:
    def __init__(s, a):
        s.tree, s.target, s.out = a.tree, a.target, a.out
        s.exp = tomllib.load(open(GATES_TOML, "rb"))
        s.env = dict(os.environ, CARGO_TARGET_DIR=s.target)
        s.rows = []
        s.bake_groups = a.bake_groups or []
        s.parity_allow = getattr(a, "parity_allow", None) or ""
        s.keep_goldens = a.keep_goldens
        s.golden_arg = getattr(a, "golden_groups", None) or "auto"
        s.base = getattr(a, "base", None) or "main"
        s.leak_plan = getattr(a, "leak_plan", None) or "always"
        s.fidelity_dir = getattr(a, "fidelity_dir", None)
        s.fidelity_confirm_dir = getattr(a, "fidelity_confirm_dir", None)
        s.fidelity_both = bool(getattr(a, "fidelity_both", False))
        fa = getattr(a, "fidelity_allow", None)
        s.fidelity_allow = None if fa is None else {x for x in re.split(r"[,\s]+", fa) if x}
        s.alignment_reference = getattr(a, "alignment_reference", None) or os.environ.get("OPENBF_ALIGNMENT_REFERENCE")
        s.alignment_cases = getattr(a, "alignment_cases", None) or shlex.split(os.environ.get("OPENBF_ALIGNMENT_CASES", ""))
        s.moved_goldens = []
        os.makedirs(s.out, exist_ok=True)

    def row(s, gate, cmd, rc, value, expected, verdict):
        s.rows.append(dict(gate=gate, cmd=cmd, exit=rc, value=value, expected=expected, verdict=verdict))
        print(f"[gates] {gate:8s} {verdict:6s} {value}", flush=True)

    def p(s, *x):
        return os.path.join(s.out, *x)

    def g_alignment(s):
        reference = getattr(s, "alignment_reference", None)
        cases = getattr(s, "alignment_cases", None) or []
        if not reference or not cases:
            s.row("alignment", "render_collision_check.py", 2, "reference and source bakes required",
                  "--alignment-reference ASSETS --alignment-case NAME=ASSETS", "FAIL")
            return
        cmd = [sys.executable, os.path.join(s.tree, "scripts/render_collision_check.py"),
               "--reference", reference, "--out", s.p("alignment.json"), "--table", s.p("alignment.csv")]
        for case in cases:
            cmd += ["--case", case]
        if os.path.isfile(s.p("alignment.json")):
            os.remove(s.p("alignment.json"))
        rc = sh(shlex.join(cmd), s.tree, s.env, s.p("alignment.log"))
        value, valid, clean = "missing alignment report", False, False
        try:
            from render_collision_check import report_problems
            with open(s.p("alignment.json")) as receipt:
                report = json.load(receipt)
            problems = report_problems(report, reference, cases)
            if problems:
                value = "invalid report: " + "; ".join(problems[:3])
            else:
                valid = True
                offenders = sum(row["counts"].get("offender", 0) for row in report["maps"])
                missing = sum(row["counts"].get("unmeasured", 0) for row in report["maps"])
                extras = sum(row["extra_source_placements"] for row in report["maps"])
                incomplete = sum(row.get(name, {}).get('complete') is not True or row.get(name, {}).get('status') != 'pass'
                                 for row in report['maps'] for name in ('terrain', 'attachment'))
                clean = not (offenders or missing or extras or incomplete or any(row.get("error") for row in report["maps"]))
                value = f"{len(report['maps'])} source/maps, {offenders} offending placements, {missing} unmeasured, {extras} extra, {incomplete} terrain/attachment gaps"
        except (OSError, ValueError, TypeError, KeyError) as exc:
            value = "invalid/missing alignment report: " + str(exc)
        s.row("alignment", shlex.join(cmd), rc, value, "complete report; 0 offenders; 0 unmeasured; 0 extras",
              "PASS" if rc == 0 and valid and clean else "FAIL")

    def build_release(s, *examples):
        """The release `game` binary plus only the named examples (card WEARX, SSD wear): `--examples` linked all ~27 of
        the crate's, each a whole engine (3.1 GB written per build after a core edit, 0.78 GB for the four the gates run).
        Each gate asks for its own; a later call links only what no earlier one built, so nothing links twice in a run.
        Another example is built by whoever runs it (`cargo build --release -p game --example <name>`)."""
        built = s.__dict__.setdefault("_built", set())
        if getattr(s, "_build_rc", 0):
            return s._build_rc
        want = [x for x in ("game",) + examples if x not in built]
        if not want:
            return 0
        args = " ".join(["--bins"] * ("game" in want) + [f"--example {x}" for x in want if x != "game"])
        cmd = f"cargo build --release -p game {args}"
        log = "build_release.txt" if not built else f"build_release_{'_'.join(x for x in want if x != 'game')}.txt"
        rc = sh(cmd, os.path.join(s.tree, "engine"), s.env, s.p(log))
        if rc:
            s._build_rc = rc
            s.row("build", cmd, rc, f"FAILED — see {log}", "", "FAIL")
        else:
            built.update(want)
        return rc

    def g_ci(s):
        rc = sh("./ci.sh", os.path.join(s.tree, "engine"), s.env, s.p("ci.txt"), lock=True)
        txt = open(s.p("ci.txt"), errors="replace").read()
        n = ci_counts(txt)
        e = s.exp["ci"]
        val = f"{n[0]} / {n[1]} / {n[2]}"
        exp = f"{e['passed']} / {e['failed']} / {e['ignored']}"
        verdict = "FAIL" if rc or n[1] else ("PASS" if val == exp else "MOVED")
        s.row("ci", "./ci.sh", rc, val, exp, verdict)

    def golden_plan(s):
        """(groups, why): the golden groups this run regenerates — see engine/goldens/deps.toml."""
        if s.golden_arg == "none":
            return [], "--golden-groups none"
        if s.golden_arg not in ("auto", "all"):
            return [g for g in s.golden_arg.split(",") if g], "--golden-groups"
        return plan_goldens(s.tree, s.base, s.golden_arg == "all")

    def g_gpu(s):
        """Render GPU tests, then REGENERATE the goldens a change can move (deps.toml): the moved ones are listed,
        quantified (before anyone looks) and put on before|after|diff sheets; then restored from HEAD unless
        --keep-goldens. A change that cannot move one skips both steps."""
        groups, why = s.golden_plan()
        save_json(s.p("goldens_plan.json"), dict(groups=groups, why=why))
        if not groups:
            s.row("gpu", "goldens skipped (engine/goldens/deps.toml)", 0, f"no golden can move: {why}", "goldens unchanged", "PASS")
            s.moved_goldens = []
            return
        print(f"[gates] goldens: {' '.join(groups)} ({why})", flush=True)
        rc1 = sh(f"{s.tree}/scripts/gpu_tests.sh --render", s.tree, s.env, s.p("gpu_render.txt"), gpu=True)
        rc2 = sh(f"{s.tree}/scripts/goldens.sh {' '.join(groups)}", s.tree, s.env, s.p("goldens_regen.txt"), gpu=True)
        st = subprocess.run(["git", "-C", s.tree, "status", "--porcelain", "--", "engine/goldens"], capture_output=True, text=True).stdout
        moved = [l[3:].strip() for l in st.splitlines() if l.strip()]
        if moved:
            q = golden_report(s.tree, moved, s.p("goldens"))
            print(f"[gates] goldens moved: {len(moved)} — numbers in goldens/QUANT.md, sheets in goldens/")
            if not s.keep_goldens:
                for rel in moved:
                    restore_from_head(s.tree, rel)
        wrong, note = core_link_check(s.tree, s.target, groups, s.p("goldens_core_link.txt"))
        print(f"[gates] goldens: {note}", flush=True)
        verdict = "FAIL" if (rc1 or rc2 or wrong) else ("MOVED" if moved else "PASS")
        if not (rc1 or rc2 or wrong) and set(groups) == set(GOLDEN_GROUPS):
            mark_full(s.tree)
        value = "goldens unchanged" if not moved else f"{len(moved)} moved: " + ", ".join(m.replace('engine/goldens/', '') for m in moved)
        if wrong:
            value = f"deps.toml skips core::{', core::'.join(wrong)}, which a golden test links — un-skip it; " + value
        s.row("gpu", f"gpu_tests.sh --render + goldens.sh {' '.join(groups)}", rc1 or rc2, value, "goldens unchanged", verdict)
        s.moved_goldens = moved

    def g_sim(s):
        if s.build_release():
            return
        env = dict(s.env, ROOT=s.tree, TARGET=s.target, OUT=s.p("sim"), BASE=s.exp["motion_base"])
        rc = sh(os.path.join(s.tree, "scripts/sim_gates.sh"), s.tree, env, s.p("sim_gates.txt"))
        txt = open(s.p("sim_gates.txt"), errors="replace").read()
        h = re.findall(r"sim hash after (\d+) ticks: ([0-9a-f]{16})", txt)
        got = {t: v for t, v in h}
        e = s.exp["sim"]
        val = f"walk {got.get('407','?')} aim {got.get('120','?')} 600 {got.get('600','?')}"
        exp = f"walk {e['walk']} aim {e['aim']} 600 {e['hash600']}"
        motion_ok = "walk/motion.csv identical" in txt and "aim/motion.csv identical" in txt
        verdict = "PASS" if (val == exp and motion_ok and rc == 0) else ("FAIL" if not h else "MOVED")
        s.row("sim", "scripts/sim_gates.sh (BASE=motion_base)", rc, val + (" motion identical" if motion_ok else " motion DIFFERS"), exp + " motion identical", verdict)
        rc2 = sh(f"{s.target}/release/game --sim-hash 100", os.path.join(s.tree, "engine"), s.env, s.p("sim_hash_100.txt"))
        v = (open(s.p("sim_hash_100.txt")).read().strip().splitlines() or ["?"])[-1].strip()
        s.row("simhash", "game --sim-hash 100", rc2, v, e["hash100"], "PASS" if v == e["hash100"] else ("FAIL" if rc2 else "MOVED"))

    def _pair(s, name, cmd, baseline):
        """One run of the example `cmd` names first. Identical to the baseline = PASS (an output that byte-matches an accepted one is the determinism
        evidence). Anything else runs it a SECOND time to tell a real move (pair identical) from a nondeterministic
        run (pair differs = FAIL), as the always-twice gate did."""
        if s.build_release(cmd.split()[0]):
            return
        ex = f"{s.target}/release/examples/"
        eng = os.path.join(s.tree, "engine")
        rcs = [run_split(ex + cmd, eng, s.env, s.p(f"{name}_1.txt"), s.p(f"{name}_1.err"))]
        base = same(s.p(f"{name}_1.txt"), baseline)
        if rcs[0] == 0 and base:
            s.row(name, cmd, 0, "vs baseline identical (single run)", "vs baseline identical", "PASS")
            return base
        if rcs[0] == 0:
            rcs.append(run_split(ex + cmd, eng, s.env, s.p(f"{name}_2.txt"), s.p(f"{name}_2.err")))
        pair = len(rcs) == 2 and same(s.p(f"{name}_1.txt"), s.p(f"{name}_2.txt"))
        rc = max(rcs)
        verdict = "FAIL" if rc or not pair else "MOVED"
        val = f"pair {'identical' if pair else 'DIFFERS'}; vs baseline DIFFERS"
        s.row(name, cmd + " ×2", rc, val, "vs baseline identical", verdict)
        return base

    def g_sweep(s):
        base = s._pair("sweep", "round_to_outcome 60000 42", os.path.join(s.exp["baseline_dir"], "sweep_main_1.txt"))
        if base is False and os.path.exists(s.p("sweep_1.txt")):
            old = open(os.path.join(s.exp["baseline_dir"], "sweep_main_1.txt")).read().splitlines()
            new = open(s.p("sweep_1.txt")).read().splitlines()
            om = {l.split()[0]: l for l in old if l.strip()}
            moved = [l for l in new if l.strip() and om.get(l.split()[0]) != l]
            open(s.p("sweep_moved_rows.txt"), "w").write("\n".join(moved) + "\n")
            print(f"[gates] sweep rows that moved: {len(moved)} (sweep_moved_rows.txt)")
        # (`sweep` is in land.py's list whenever core|game changed, so the fidelity rows ride here.) A sweep that is byte-identical
        # to main's - every map's whole round at seed 42 - is a change that moved no bot: the 8-seed fidelity run (3-4 min) would
        # measure main again. 1 of 9 gate runs on 2026-10-09 (the black box's schema 4). An explicit `--groups fidelity` still runs.
        if base is True and "fidelity" not in getattr(s, "asked", ()) and (s.exp.get("fidelity") or {}).get("enabled", True):
            s._fid_done = True
            s.row("fidelity", "skipped: the sweep is byte-identical to main's", 0, "no behaviour change to measure", "", "PASS")
            return
        s.g_fidelity()

    def g_arena(s):
        b = s.exp["baseline_dir"]
        s._pair("arena_flat", "arena 3000 42 flat all", os.path.join(b, "arena_flat_1.txt"))
        s._pair("arena_tat2", "arena 9000 42 tat2 all", os.path.join(b, "arena_tat2_1.txt"))

    def g_leak(s):
        s.g_ordnance()   # 30 s: runs whatever the leak plan says (leak_deps.toml skips its example's own edits)
        if s.leak_plan == "auto":
            run_it, why = plan_leak(s.tree, s.base)
            if not run_it:
                s.row("leak", "leak_census skipped (engine/goldens/leak_deps.toml)", 0, f"no census input moved: {why}", "vs baseline identical", "PASS")
                return
            print(f"[gates] leak: {why}", flush=True)
        n0 = len(s.rows)
        s._pair("leak", "leak_census", os.path.join(s.exp["baseline_dir"], "leak_main_1.txt"))
        if len(s.rows) > n0 and all(r["verdict"] != "FAIL" for r in s.rows[n0:]) and (
                not os.path.exists(s.p("leak_1.txt")) or "new 0" in open(s.p("leak_1.txt")).read().splitlines()[-1]):
            mark_full(s.tree, LEAK_MARK)
        if os.path.exists(s.p("leak_1.txt")):
            foot = [l for l in open(s.p("leak_1.txt")).read().splitlines() if l.startswith("leak:")]
            m = re.search(r"new (\d+)", foot[-1]) if foot else None
            if not m or int(m.group(1)):
                s.row("leak_new", "leak_census footer", 0, foot[-1] if foot else "no footer", "new 0", "FAIL")

    def g_ordnance(s):
        """ordnance_census (card GRFL / GRFG), once, with the leak group: its footer must end `new 0` (every grenade
        that passed a soldier's floor is on game/examples/ordnance_census_accepted.txt). The other counts (floors, runs)
        follow the nav graph and the bake, so a footer unlike gates.toml's `[ordnance] footer` with new 0 still passes,
        its value says so; the planner re-takes the line."""
        want = s.exp.get("ordnance", {}).get("footer")
        if not want:
            s.row("ordnance", "ordnance_census skipped", 0, "no [ordnance] footer in gates.toml", "", "PASS")
            return
        if s.build_release("ordnance_census"):
            return
        exe = f"{s.target}/release/examples/ordnance_census"
        rc = run_split(exe, os.path.join(s.tree, "engine"), s.env, s.p("ordnance_1.txt"), s.p("ordnance_1.err"))
        foot = [l for l in open(s.p("ordnance_1.txt"), errors="replace").read().splitlines() if l.startswith("ordnance:")]
        got = foot[-1].strip() if foot else "no footer - see ordnance_1.txt / .err"
        ok = rc == 0 and re.search(r"\bnew 0$", got) is not None
        if ok and got != want:
            got += " (counts moved from gates.toml's line, new 0)"
        s.row("ordnance", "ordnance_census", rc, got, want, "PASS" if ok else "FAIL")

    def g_bake(s):
        bake_bin = f"{s.target}/release/bake"
        env = dict(s.env, ROOT=s.tree, OUT=s.p("bake"))
        # Build the tree's OWN bake first: `build_release` builds only `-p game`, and a target seeded from main (reflink)
        # already holds main's binary, which bakes main's output and compares equal (found by LITEB, 2026-09-25).
        if sh("cargo build --release -p bake", os.path.join(s.tree, "engine"), s.env, s.p("build_bake.txt")):
            s.row("bake", "cargo build --release -p bake", 1, "FAILED — see build_bake.txt", "", "FAIL")
            return
        if os.access(bake_bin, os.X_OK):
            env["BAKE"] = bake_bin
        # The declared groups alone miss what the same code writes in another group, so a diff in the bake code bakes all.
        full = bool(s.bake_groups) and touches_bake(changed_files(s.tree, s.base))
        grp = "" if full else " ".join(s.bake_groups)
        rc = sh(f"{s.tree}/scripts/scratch_bake.sh {grp}", s.tree, env, s.p("scratch_bake.txt"), lock=True)
        rc2 = sh(f"{REPO}/scripts/cmp_bake.sh {s.p('bake')}", s.tree, dict(os.environ, ROOT=REPO), s.p("cmp_bake.txt"))
        txt = open(s.p("cmp_bake.txt"), errors="replace").read()
        m = re.search(r"SAME=(\d+) DIFF=(\d+) ONLY_IN_A=(\d+) ONLY_IN_B=(\d+)", txt)
        if rc or not m:
            s.row("bake", f"scratch_bake.sh {grp} + cmp_bake.sh", rc or rc2, "FAILED — see scratch_bake.txt / cmp_bake.txt", "", "FAIL")
            return
        verdict, val, exp, stray = bake_verdict(txt, s.bake_groups, full, s.exp["bake"]["files"])
        if stray:
            open(s.p("bake_undeclared.txt"), "w").write("".join(f"{g}\t{f}\n" for f, g in stray))
        s.row("bake", f"scratch_bake.sh {grp or '(all)'} + cmp_bake.sh", rc, val, exp, verdict)

    def g_parity(s):
        """The PS2 parity score (scripts/parity.py gate): the tree vs main HEAD, per map. It takes ci.lock + gpu.lock
        itself, around the rendering only. Exit 3 = main has no pose_render yet (no baseline): PASS, said so."""
        allow = f" --allow {s.parity_allow}" if s.parity_allow else ""
        cmd = f"{sys.executable} {REPO}/scripts/parity.py gate --tree {s.tree} --target {s.target} --out {s.p('parity')}{allow}"
        rc = sh(cmd, s.tree, s.env, s.p("parity.txt"))
        try:
            j = json.load(open(s.p("parity", "parity.json")))
        except (OSError, ValueError):
            j = None
        exp = "no map +0.5 past main" + (f" (declared: {s.parity_allow})" if s.parity_allow else "")
        if rc == 3 and j and j.get("status") == "no-baseline":
            s.row("parity", "parity.py gate", 0, f"tree {j['tree']:.3f}; main has no pose_render — no baseline", exp, "PASS")
            return
        if not j or rc not in (0, 1):
            s.row("parity", "parity.py gate", rc, "FAILED — see parity.txt", exp, "FAIL")
            return
        rows = j["rows"]
        past = [f"{r['map']} {r['delta']:+.2f}" for r in rows if r["verdict"] in ("FAIL", "ALLOWED")]
        better = [f"{r['map']} {r['delta']:+.2f}" for r in rows if r["delta"] < -0.5]
        val = (f"mean block error {j['tree']:.3f} (main {j['main']:.3f})" + (f"; rose: {', '.join(past)}" if past else "")
               + (f"; improved: {', '.join(better)}" if better else "") + " — parity/parity.md, parity/tree/worst.png")
        verdict = "FAIL" if rc == 1 else ("MOVED" if past else "PASS")
        s.row("parity", "parity.py gate", rc, val, exp, verdict)

    def g_protect(s):
        """card FIDL: the oracle (scripts/oracle_guard.py) is not changed by a card that did not declare it."""
        if getattr(s, "_protect_done", False):
            return
        s._protect_done = True
        sys.path.insert(0, os.path.join(REPO, "scripts"))
        import oracle_guard
        cmd = f"git diff {s.base}...HEAD (oracle paths)"
        try:
            ch = oracle_guard.changed_by_class(s.tree, s.base)
        except oracle_guard.ComparisonError as e:
            s.row("protect", cmd, 1, str(e), "successful protected-path comparison", "FAIL")
            return
        cid, ok = oracle_guard.card_declares_for(os.environ, s.tree)
        if not ch:
            s.row("protect", cmd, 0, "no protected path changed", "none changed", "PASS")
            return
        undecl = {k: v for k, v in ch.items() if k not in ok}
        lst = "; ".join(f"{k}: {', '.join(v[:3])}{' +%d' % (len(v) - 3) if len(v) > 3 else ''}" for k, v in sorted(ch.items()))
        if undecl:
            s.row("protect", cmd, 1, f"UNDECLARED change of the yardstick - {lst} (card {cid or '?'}; declare "
                  f"`oracle=<{'|'.join(sorted(undecl))}>` on the card if intended)", "none changed", "FAIL")
        else:
            s.row("protect", cmd, 0, f"declared by {cid}: {lst}", "none changed", "MOVED")

    def g_fidelity(s):
        """card FIDL: fidelity numbers vs gates.toml [fidelity]; see the module docstring."""
        if getattr(s, "_fid_done", False):
            return
        s._fid_done = True
        cfg = s.exp.get("fidelity")
        if not cfg or not cfg.get("enabled", True):
            s.row("fidelity", "gates.toml [fidelity]", 0, "not checked: fidelity disabled or unconfigured", "", "PASS")
            return
        d = s.fidelity_dir
        if not d:
            d = s.p("fidelity")
            if s.build_release("round_to_outcome"):
                return
            cmd = " ".join([sys.executable, os.path.join(s.tree, "scripts/sweep_seeds.py"), "--out", d, "--seeds",
                            str(cfg.get("seeds", "1-8")), "--ticks", str(cfg.get("ticks", 60000)), "--target", s.target,
                            "--botq", "--jobs", str(cfg.get("jobs", 8))])
            rc = run_split(cmd, s.tree, s.env, s.p("fidelity_run.txt"), s.p("fidelity_run.err"))
            if rc:
                s.row("fidelity", "sweep_seeds.py --botq", rc, "the sweep FAILED - see fidelity_run.txt / .err", "", "FAIL")
                return
        try:
            new = fidelity_measure(d)
        except Exception as e:  # a run with no rows must FAIL, not pass silently
            s.row("fidelity", "fidelity_measure", 1, f"could not read {d}: {e}", "", "FAIL")
            return
        os.makedirs(s.p("fidelity"), exist_ok=True)
        json.dump(new, open(s.p("fidelity", "fidelity_new.json"), "w"), indent=1, sort_keys=True)
        missing = fidelity_missing(cfg, new)
        if missing:
            s.row("fidelity", "fidelity_measure", 1, "missing or nonfinite required values: " + ", ".join(missing),
                  "all configured non-track baseline values measured", "FAIL")
            return
        if s.fidelity_both and cfg.get("confirm_seeds") and not s.fidelity_dir:   # a baseline run: main on the confirm seeds too
            d2 = s.p("fidelity_confirm")
            cmd2 = " ".join([sys.executable, os.path.join(s.tree, "scripts/sweep_seeds.py"), "--out", d2, "--seeds",
                             str(cfg["confirm_seeds"]), "--ticks", str(cfg.get("ticks", 60000)), "--target", s.target,
                             "--botq", "--jobs", str(cfg.get("jobs", 8))])
            receipt = s.p("fidelity", "fidelity_confirm_new.json")
            if os.path.exists(receipt):
                os.remove(receipt)   # a failed requested block must not retain a prior baseline receipt
            try:
                rc = run_split(cmd2, s.tree, s.env, s.p("fidelity_confirm_run.txt"), s.p("fidelity_confirm_run.err"))
                if rc:
                    s.row("fidelity", "sweep_seeds.py --botq (required confirm block)", rc,
                          "the requested confirm sweep FAILED - see fidelity_confirm_run.txt / .err", "", "FAIL")
                    return
                new2 = fidelity_measure(d2)
                missing = fidelity_missing(cfg, new2)
                if missing:
                    s.row("fidelity", "fidelity_measure (required confirm block)", 1,
                          "missing or nonfinite required confirm values: " + ", ".join(missing),
                          "all configured non-track baseline values measured", "FAIL")
                    return
            except Exception as e:
                s.row("fidelity", "required confirm block", 1, f"could not execute/read {d2}: {e}", "", "FAIL")
                return
            json.dump(dict(seeds=str(cfg["confirm_seeds"]), values=new2),
                      open(receipt, "w"), indent=1, sort_keys=True)
        allow = s.fidelity_allow
        if allow is None:
            allow, void = exposed_filter(cfg, card_fidelity_allow(s.tree), card_exposes(s.tree))
            for t in void:
                print(f"[gates] fidelity: the approval of {t} is void - it is marked `exposes`, and main's own baseline there is 0 "
                      f"(a fault main does not show was not exposed, it was caused)", flush=True)
        res = fidelity_compare(cfg, new, allow)
        dropped = []
        if res["regress"] and cfg.get("confirm_seeds") and (not s.fidelity_dir or s.fidelity_confirm_dir):   # per-map counts and
            d2 = s.fidelity_confirm_dir or s.p("fidelity_confirm")                                             # sums are noisy: look again
            cmd2 = " ".join([sys.executable, os.path.join(s.tree, "scripts/sweep_seeds.py"), "--out", d2, "--seeds",
                             str(cfg["confirm_seeds"]), "--ticks", str(cfg.get("ticks", 60000)), "--target", s.target,
                             "--botq", "--jobs", str(cfg.get("jobs", 8))])
            if s.fidelity_confirm_dir or run_split(cmd2, s.tree, s.env, s.p("fidelity_confirm_run.txt"), s.p("fidelity_confirm_run.err")) == 0:
                try:
                    new2 = fidelity_measure(d2)
                    if not fidelity_missing(cfg, new2):
                        res["regress"], dropped = fidelity_confirm(cfg, res, new2, confirm_base(cfg))
                except Exception:
                    pass   # no usable second block: the first verdict stands
        open(s.p("fidelity", "fidelity.md"), "w").write(fidelity_report(res, cfg) + fidelity_dropped_note(dropped, cfg))
        exp = "no metric worse than its baseline past tolerance" + (f" (declared: {','.join(sorted(allow))})" if allow else "")
        bad, ok_moved, better = res["regress"], res["declared"], res["better"]

        def fmt(items, n=6):
            t = [f"{m}{'@' + k if k else ''} {b:.2f}->{v:.2f}" for m, k, b, v in items[:n]]
            return ", ".join(t) + (f" +{len(items) - n} more" if len(items) > n else "")
        val = (f"{res['checked']} values vs baseline ({cfg.get('source', '?')})" if res["checked"] else
               "not checked: no configured non-track baseline values")
        if bad:
            val += f"; WORSE undeclared: {fmt(bad)}"
        if ok_moved:
            val += f"; worse, declared: {fmt(ok_moved)}"
        if better:
            val += f"; better: {len(better)}"
        if dropped:
            val += f"; worse on seeds {cfg['seeds']} but NOT on {cfg['confirm_seeds']} (noise, not failed): {len(dropped)}"
        verdict = "FAIL" if bad else ("MOVED" if ok_moved else "PASS")
        s.row("fidelity", "sweep_seeds.py --botq + gates.toml [fidelity]", 1 if bad else 0, val + " - fidelity/fidelity.md", exp, verdict)

    def g_map(s):
        s.g_protect()   # land.py passes its own fixed group list and `map` is always in it, so the oracle diff rides here
        rc = sh(f"{s.tree}/scripts/size_report.sh --check-map", s.tree, s.env, s.p("size_report.txt"))
        s.row("map", "scripts/size_report.sh --check-map", rc, "ok" if rc == 0 else "FAILED — see size_report.txt", "ok", "PASS" if rc == 0 else "FAIL")

    def write(s):
        L = ["| gate | command | exit | value | expected | verdict |", "|---|---|---|---|---|---|"]
        for r in s.rows:
            L.append(f"| {r['gate']} | `{r['cmd']}` | {r['exit']} | {r['value']} | {r['expected']} | **{r['verdict']}** |")
        lost = [r["value"] for r in s.rows if r["verdict"] == ADAPTER_LOST]
        open(s.p("SUMMARY.md"), "w").write("".join(f"{v}\n\n" for v in lost) + f"Gates on `{s.tree}` (outputs in `{s.out}`)\n\n" + "\n".join(L) + "\n")
        json.dump(s.rows, open(s.p("results.json"), "w"), indent=1)
        print("\n".join(L))


def restore_from_head(tree, rel):
    """Rewrite a file with its HEAD content (the guard hook denies `git checkout -- <path>`); delete it if HEAD lacks it."""
    r = subprocess.run(["git", "-C", tree, "show", f"HEAD:{rel}"], capture_output=True)
    path = os.path.join(tree, rel)
    if r.returncode == 0:
        open(path, "wb").write(r.stdout)
    elif os.path.exists(path):
        os.remove(path)


def golden_report(tree, moved, outdir):
    """Quantify every moved golden against HEAD and write before|after|diff sheets. Returns the rows."""
    from PIL import Image, ImageChops, ImageDraw
    import io
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for i, rel in enumerate(moved):
        if not rel.endswith(".png"):
            # The walkabout referee's statistics (`goldens/walkabout_stats.ron`, goldens.sh): not a picture; it
            # moves with the images and is judged by them.
            rows.append((rel, "not an image", "", "", ""))
            continue
        new = Image.open(os.path.join(tree, rel)).convert("RGBA")
        g = subprocess.run(["git", "-C", tree, "show", f"HEAD:{rel}"], capture_output=True)
        if g.returncode or not g.stdout:
            rows.append((rel, "NEW (not in HEAD)", "", "", ""))
            continue
        old = Image.open(io.BytesIO(g.stdout)).convert("RGBA")
        if old.size != new.size:
            rows.append((rel, f"SIZE {old.size} -> {new.size}", "", "", ""))
            continue
        d = ImageChops.difference(old, new)
        px = list(d.getdata())
        n = len(px)
        changed = sum(1 for p in px if any(p))
        mean = sum(sum(p[:3]) for p in px) / (3 * n)
        mx = max(max(p) for p in px)
        bbox = d.convert("RGB").getbbox()  # an RGBA getbbox looks at alpha only
        rows.append((rel, f"{mean:.3f}", str(mx), f"{100 * changed / n:.2f} %", str(bbox)))
        w = old.width
        sheet = Image.new("RGB", (w * 3, old.height + 18), (30, 30, 30))
        dd = ImageDraw.Draw(sheet)
        amp = ImageChops.multiply(d.convert("RGB"), Image.new("RGB", d.size, (255, 255, 255)))
        amp = amp.point(lambda v: min(255, v * 8))
        for k, (im, lab) in enumerate(((old, "HEAD"), (new, "regenerated"), (amp, "diff x8"))):
            sheet.paste(im.convert("RGB"), (k * w, 18))
            dd.text((k * w + 4, 3), f"{lab}: {rel.replace('engine/goldens/', '')}", fill=(255, 255, 0))
        sheet.save(os.path.join(outdir, f"pair_{i:02d}_{os.path.basename(rel)}"))
    L = ["| golden | mean abs | max | changed | bbox |", "|---|---|---|---|---|"] + [f"| {' | '.join(r)} |" for r in rows]
    open(os.path.join(outdir, "QUANT.md"), "w").write("\n".join(L) + "\n")
    return rows


GOLDEN_GROUPS = ["tat1", "hud", "shell", "spawn", "gallery", "walkabout"]  # scripts/goldens.sh's groups
FULL_MARK = os.path.join(REPO, "engine/artifacts/loop/goldens_full.sha")
LEAK_MARK = os.path.join(REPO, "engine/artifacts/loop/leak_full.sha")  # the last clean census run (card GSPD)
MAIN_MARK = os.path.join(REPO, "engine/artifacts/loop/main_full.sha")  # land.py: the last landing whose main gates all ran


def ci_counts(txt):
    """[passed, failed, ignored] from ci.sh's output. Under cargo-nextest (card TSTX) its `Summary` line carries them -
    `N tests run: P passed, F failed, S skipped` (skipped = the #[ignore]d tests; ci.sh filters nothing) - and the
    doctests' `cargo test --doc` adds its own `test result:` lines; under plain `cargo test` only those lines exist."""
    n = [0, 0, 0]
    for m in re.finditer(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", txt):
        for i in range(3):
            n[i] += int(m.group(i + 1))
    m = None
    for m in re.finditer(r"Summary \[[^\]]*\] (\d+) tests? run: ([^\n]*)", txt):
        pass
    if m:
        part = dict((k, int(v)) for v, k in re.findall(r"(\d+) (passed|failed|skipped|timed out)", m.group(2)))
        n[0] += part.get("passed", 0)
        n[1] += part.get("failed", 0) + part.get("timed out", 0)
        n[2] += part.get("skipped", 0)
    return n


def _git(tree, *a):
    r = subprocess.run(["git", "-C", tree, *a], capture_output=True, text=True)
    return r.returncode, r.stdout.strip()


def can_move_golden(f, d):
    """False when file f (repo-relative) cannot move a golden by deps.toml's rules."""
    if any(f.startswith(p) for p in d.get("always", [])):
        return True
    if any(f.endswith(x) for x in d.get("skip_suffix", [])):
        return False
    return not any(f.startswith(p) for p in d.get("skip", []) + d.get("skip_tests", []))


GOLDEN_TESTS = dict(tat1="golden_tat1", hud="golden_hud", shell="golden_shell", spawn="golden_spawn",
                    gallery="golden_gallery", walkabout="walkabout_stats")  # scripts/goldens.sh's group -> test binary
CORE_SRC = "engine/crates/core/src/"


def core_modules(tree):
    """The `core` crate's top-level modules: every core/src/<m>.rs and core/src/<m>/."""
    try:
        names = os.listdir(os.path.join(tree, CORE_SRC))
    except OSError:
        return []
    return sorted({n[:-3] if n.endswith(".rs") else n for n in names} - {"lib"})


def linked_core(nm_text, mods):
    """{module: number of its functions} among `nm -C --defined-only` text symbols. A golden test is a dev-profile
    binary, so a `core` function it can reach is in it under its own name (`core::door::sample`,
    `<core::heightfield::Heightfield>::height_at`) and the linker's --gc-sections has dropped the ones it cannot."""
    if not mods:
        return {}
    pat = re.compile(r"^<?core::(" + "|".join(map(re.escape, mods)) + r")::(.*)")
    got = {}
    for line in nm_text.splitlines():
        p = line.split(" ", 2)
        if len(p) < 3 or p[1] not in "tTwW":
            continue
        m = pat.match(p[2])
        if m and not (m.group(1) == "unit" and m.group(2).startswith("IsUnit")):  # std's own core::unit::IsUnit
            got[m.group(1)] = got.get(m.group(1), 0) + 1
    return got


def skipped_but_linked(linked, d):
    """The linked `core` modules that deps.toml would skip — each one is a wrong skip rule."""
    return sorted(m for m in linked
                  if not (can_move_golden(f"{CORE_SRC}{m}.rs", d) and can_move_golden(f"{CORE_SRC}{m}/x.rs", d)))


def core_link_check(tree, target, groups, out_file):
    """(wrong, note) after a golden run: which `core` modules the golden tests just built link, and whether deps.toml
    skips any of them (card GSKIP: `core` was skipped whole while the gallery's vantage stood on core::collision).
    Only the test binaries are read — the tat1 overview is the `game` binary's capture, which links all of `core`."""
    try:
        d = tomllib.load(open(os.path.join(tree, "engine/goldens/deps.toml"), "rb"))
    except (OSError, ValueError):
        return [], "deps.toml unreadable"
    mods, linked, lines = core_modules(tree), {}, []
    for g in groups:
        bins = [b for b in glob.glob(os.path.join(target, "debug/deps", GOLDEN_TESTS[g] + "-*")) if "." not in os.path.basename(b)]
        if not bins:
            lines.append(f"{g}: no test binary under {target}/debug/deps")
            continue
        try:
            r = subprocess.run(["nm", "-C", "--defined-only", max(bins, key=os.path.getmtime)], capture_output=True, text=True)
        except OSError as e:
            return [], f"nm unavailable ({e})"
        got = linked_core(r.stdout, mods) if r.returncode == 0 else {}
        lines.append(f"{g}: " + (", ".join(f"{m} {n}" for m, n in sorted(got.items())) or "none"))
        for m, n in got.items():
            linked[m] = linked.get(m, 0) + n
    wrong = skipped_but_linked(linked, d)
    lines.append("skipped by deps.toml but linked: " + (", ".join(wrong) or "none"))
    open(out_file, "w").write("\n".join(lines) + "\n")
    return wrong, f"core linked: {', '.join(sorted(linked)) or 'none'}"


def mark_full(tree, mark=FULL_MARK):
    """A full run (every golden group, or every gate on main) just finished clean on `tree`: remember its HEAD."""
    rc, sha = _git(tree, "rev-parse", "HEAD")
    if rc == 0:
        try:
            open(mark, "w").write(sha + "\n")
        except OSError:
            pass


def full_due(tree, every, mark=FULL_MARK):
    """True when the last full run is unknown / not an ancestor of HEAD, or `every` commits touching engine/crates have
    landed since."""
    try:
        sha = open(mark).read().strip()
    except OSError:
        return True
    if _git(tree, "merge-base", "--is-ancestor", sha, "HEAD")[0] != 0:
        # The mark is a branch tip that did not land (a blocked card's gate run) or was rebased since. It was "due" every
        # time until a landing rewrote the mark - 4 of 7 censuses and 3 full golden runs on 2026-10-09. Count from where
        # that branch left this history instead: older than the mark, so never later than the true count.
        rc, sha = _git(tree, "merge-base", sha, "HEAD")
        if rc != 0 or not sha:
            return True
    rc, n = _git(tree, "rev-list", "--count", f"{sha}..HEAD", "--", "engine/crates")
    return rc != 0 or int(n or 0) >= every


def plan_goldens(tree, base, force_all=False):
    """(groups, why) for the gpu group's golden step, from engine/goldens/deps.toml and the files changed since base."""
    path = os.path.join(tree, "engine/goldens/deps.toml")
    if not os.path.exists(path):
        path = os.path.join(REPO, "engine/goldens/deps.toml")
    try:
        d = tomllib.load(open(path, "rb"))
    except (OSError, ValueError):
        return list(GOLDEN_GROUPS), "deps.toml unreadable — all groups"
    if force_all:
        return list(GOLDEN_GROUPS), "--golden-groups all"
    rc, out = _git(tree, "diff", "--name-only", f"{base}...HEAD")
    if rc:
        return list(GOLDEN_GROUPS), f"git diff {base}...HEAD failed — all groups"
    if _git(tree, "rev-parse", base)[1] == _git(tree, "rev-parse", "HEAD")[1]:
        return list(GOLDEN_GROUPS), f"HEAD is {base}: no diff to judge by — all groups (land.py passes --golden-groups)"
    files = out.split()
    live = [f for f in files if can_move_golden(f, d)]
    if full_due(tree, int(d.get("full_every", 10))):
        return list(GOLDEN_GROUPS), f"full regeneration due (>= {d.get('full_every', 10)} crate commits since the last)"
    if not live:
        return [], f"{len(files)} changed file(s), none can move a golden"
    groups = []
    for g in GOLDEN_GROUPS:
        nb = (d.get("groups", {}).get(g, {}) or {}).get("not_by", [])
        if nb and all(any(f.startswith(p) for p in nb) for f in live):
            continue
        groups.append(g)
    return groups, f"{len(live)} of {len(files)} changed file(s) can move a golden"


def can_move_leak(f, d):
    """False when file f (repo-relative) cannot change the collision census (engine/goldens/leak_deps.toml)."""
    if any(f.startswith(p) for p in d.get("always", [])):
        return True
    if any(f.endswith(x) for x in d.get("skip_suffix", [])):
        return False
    if any(x in f for x in d.get("skip_contains", [])):
        return False
    keep = d.get("skip_examples_except")     # another example program is another binary: only the census's own files count
    if keep is not None and re.match(r"engine/crates/[^/]+/examples/", f) and os.path.basename(f) not in keep:
        return False
    return not any(f.startswith(p) for p in d.get("skip", []))


def plan_leak(tree, base, mark=None):
    """(run, why) for the `leak` gate under the `auto` plan: False when no changed file since `base` can move the census
    (leak_deps.toml) and the every-N full run is not due. Anything unreadable or unjudgeable runs it."""
    mark = mark or LEAK_MARK
    path = os.path.join(tree, "engine/goldens/leak_deps.toml")
    if not os.path.exists(path):
        path = os.path.join(REPO, "engine/goldens/leak_deps.toml")
    try:
        d = tomllib.load(open(path, "rb"))
    except (OSError, ValueError):
        return True, "leak_deps.toml unreadable"
    rc, out = _git(tree, "diff", "--name-only", f"{base}...HEAD")
    if rc:
        return True, f"git diff {base}...HEAD failed"
    if _git(tree, "rev-parse", base)[1] == _git(tree, "rev-parse", "HEAD")[1]:
        return True, f"HEAD is {base}: no diff to judge by"
    files = out.split()
    if full_due(tree, int(d.get("full_every", 10)), mark):
        return True, f"a census is due (>= {d.get('full_every', 10)} crate commits since the last clean one)"
    live = [f for f in files if can_move_leak(f, d)]
    if live:
        return True, f"{len(live)} of {len(files)} changed file(s) can move the census"
    return False, f"{len(files)} changed file(s), none can move the census"


BAKE_CODE = r"engine/crates/(mungefs|bake|manifest)/|scripts/(rebake|scratch_bake)\.sh"


# ---------- fidelity (card FIDL) ----------
# Which way is better. `down`: lower is better (a rise past tolerance = a regression); `up`: higher is better; `track`:
# recorded, never gated. Directions follow the PS2: a bot that stands still, bunches, reverses, stalls or walks in place is
# worse; one that fights within 20 m and finishes the round is better. Where a card moves one on purpose (FGHT cut
# `mate_3_pct` and `still_pct` toward the PS2's) it says so in `declares: fidelity=...`.
FID_SWEEP = ["finished_pct", "stuck_max_med", "recycled", "noprog", "world_death_pct", "wip_pct", "offgraph_pct"]
FID_BOTQ = ["still_pct", "mate_3_pct", "mate_1p5_pct", "contact20_pct", "rev_mean", "rev6_pct", "pinball_pct", "idle20_pct",
            "stand20_pct", "teamkill_pct", "deaths_per_min", "loop20"]
FID_SUM = {"recycled", "noprog", "loop20"}   # per map these are sums over the seeds and so is the "agg"; the rest are means


def _botq_one(f):
    sys.path.insert(0, os.path.join(REPO, "scripts"))
    import bot_quality, loop_spells
    metrics = bot_quality.measure(bot_quality.load_port(f), bot_quality.load_events(f[:-4] + ".ev.csv"), 40.0, 540.0)
    # RLOOP counts whole-life spells, including the opening; do not use bot_quality's 40 s warm-up cut.
    metrics["loop20"] = len(loop_spells.measure_file(f))
    return metrics


def fidelity_measure(d):
    """{metric: {"maps": {map: value}, "agg": value}} from a finished `sweep_seeds.py --botq` out dir. Thin parsing: the
    sweep's stdout/stderr rows through sweep_seeds' parsers, the 4 Hz samples through bot_quality and loop_spells."""
    sys.path.insert(0, os.path.join(REPO, "scripts"))
    import sweep_seeds, bot_quality
    run = sweep_seeds.load_run(d)
    if not run:
        raise ValueError("no sweep_s<seed>.txt")
    stats = sweep_seeds.per_map(run)
    out = {m: {"maps": {}} for m in FID_SWEEP + FID_BOTQ}
    for k, st in stats.items():
        n = st["n"] or 1
        out["finished_pct"]["maps"][k] = 100.0 * st["fin"] / n
        if st["sm"]:
            out["stuck_max_med"]["maps"][k] = float(st["sm_med"])
        if st["recycled"] is not None:
            out["recycled"]["maps"][k] = float(st["recycled"])
        if st["noprog"] is not None:
            out["noprog"]["maps"][k] = float(st["noprog"])
        if st["deaths"]:
            tot = sum(st["deaths"]) or 1
            out["world_death_pct"]["maps"][k] = 100.0 * sum(st["deaths"][1:]) / tot
        sh = st["shares"]
        if sh:
            w = sum(x["n"] for x in sh) or 1
            out["wip_pct"]["maps"][k] = 100.0 * sum(x["wip"] for x in sh) / w
            out["offgraph_pct"]["maps"][k] = sum(x["N"] * x["n"] for x in sh) / w
    bq = os.path.join(d, "botq")
    by = {}
    if os.path.isdir(bq):
        for f in sorted(os.listdir(bq)):
            if f.endswith(".csv") and not f.endswith(".ev.csv") and "_s" in f:
                by.setdefault(f.rsplit("_s", 1)[0], []).append(os.path.join(bq, f))
    flat = [f for k in sorted(by) for f in sorted(by[k])]
    if len(flat) > 4:   # one process took 8 minutes over 256 files (2026-10-11); the files are independent
        import multiprocessing
        # fork, not Python 3.14's default forkserver: that one re-imports the caller's main module
        with multiprocessing.get_context("fork").Pool(max(1, min(len(os.sched_getaffinity(0)), 16, len(flat)))) as pool:
            got = dict(zip(flat, pool.map(_botq_one, flat, chunksize=4)))
    else:
        got = {f: _botq_one(f) for f in flat}
    for k, files in by.items():
        ms = [got[f] for f in sorted(files)]
        for key in FID_BOTQ:
            v = [m[key] for m in ms if key in m and m[key] == m[key]]
            if v:
                out[key]["maps"][k] = sum(v) if key in FID_SUM else sum(v) / len(v)
    for m, rec in out.items():
        vals = list(rec["maps"].values())
        rec["agg"] = (sum(vals) if m in FID_SUM else sum(vals) / len(vals)) if vals else None
    return {m: r for m, r in out.items() if r["maps"]}


def fidelity_tol(spec, base, scope):
    """Allowed worsening for one value: max(abs, rel * |baseline|) from the spec's tol_{map,agg}_{abs,rel}."""
    return max(spec.get(f"tol_{scope}_abs", 0.0), spec.get(f"tol_{scope}_rel", 0.0) * abs(base))


def fidelity_missing(cfg, new):
    """Required finite baseline observations; partial comparison remains useful elsewhere."""
    def finite(v):
        return isinstance(v, (int, float)) and math.isfinite(v)

    missing = []
    for m, spec in cfg.get("metrics", {}).items():
        if spec.get("dir", "track") == "track":
            continue
        cur = new.get(m) or {}
        if spec.get("agg") is not None and not finite(cur.get("agg")):
            missing.append(f"{m}@agg")
        maps = cur.get("maps") or {}
        missing += [f"{m}@{k}" for k, b in spec.get("maps", {}).items() if b is not None and not finite(maps.get(k))]
    return missing


def fidelity_compare(cfg, new, allow=None):
    """Compare measured values with the baselines. allow: tokens (metric, metric@map, metric@agg, @map, all).
    Returns dict(regress=[(metric, map|'', base, new)], declared=[...], better=[...], checked=N)."""
    allow = allow or set()
    res = dict(regress=[], declared=[], better=[], checked=0)
    for m, spec in cfg.get("metrics", {}).items():
        direction = spec.get("dir", "track")
        cur = new.get(m)
        if direction == "track" or not cur:
            continue
        sgn = 1.0 if direction == "down" else -1.0   # worse = sgn * (new - base) > 0
        rows = [("", spec.get("agg"), cur.get("agg"), "agg")] + [(k, b, cur["maps"].get(k), "map") for k, b in spec.get("maps", {}).items()]
        for k, b, v, scope in rows:
            if b is None or v is None:
                continue
            res["checked"] += 1
            worse = sgn * (v - b)
            tol = fidelity_tol(spec, b, scope)
            if worse > tol:
                free = ("all" in allow or m in allow or (k and (f"{m}@{k}" in allow or f"@{k}" in allow))
                        or (not k and f"{m}@agg" in allow))
                res["declared" if free else "regress"].append((m, k, b, v))
            elif worse < -tol:
                res["better"].append((m, k, b, v))
    return res


CONFIRM_BASE = os.path.join(REPO, "engine/artifacts/lead_tmp/loop_baselines/fidelity_confirm_main.json")


def confirm_base(cfg, path=None):
    """MAIN's own values on the confirm seeds ({metric: {maps, agg}}), or None when not taken for these seeds."""
    try:
        d = json.load(open(path or CONFIRM_BASE))
    except (OSError, ValueError):
        return None
    return d.get("values") if str(d.get("seeds")) == str(cfg.get("confirm_seeds")) else None


def fidelity_confirm(cfg, res, new2, base2=None):
    """Second look at a per-map failure: keep it only if the same map is also worse past the tolerance in a second block of
    seeds (`new2`) - against MAIN's value on THOSE seeds (`base2`, card FIDC: main itself read 0.03 / 0.71 / 1.97 on three
    blocks where GRAIA's branch read 0.67 / 0.95 / 0.56), else against the seeds 1-8 baseline. An all-map failure of a SUM
    (FID_SUM, card FIDA) gets the same second look, but only against main's own confirm-block sum (no `base2` = it stands);
    an all-map MEAN is never dropped. Returns (kept, dropped)."""
    kept, dropped = [], []
    for m, k, b, v in res["regress"]:
        spec = cfg["metrics"][m]
        sgn = 1.0 if spec.get("dir") == "down" else -1.0
        if not k:
            v2 = (new2.get(m) or {}).get("agg")
            b2 = ((base2 or {}).get(m) or {}).get("agg")
            if m not in FID_SUM or v2 is None or b2 is None or sgn * (v2 - b2) > fidelity_tol(spec, b2, "agg"):
                kept.append((m, k, b, v))
            else:
                dropped.append((m, k, b, v, v2))
            continue
        v2 = (new2.get(m) or {}).get("maps", {}).get(k)
        b2 = ((base2 or {}).get(m) or {}).get("maps", {}).get(k, b)
        if v2 is None or sgn * (v2 - b2) > fidelity_tol(spec, b2, "map"):
            kept.append((m, k, b, v))
        else:
            dropped.append((m, k, b, v, v2))
    return kept, dropped


def fidelity_dropped_note(dropped, cfg):
    if not dropped:
        return ""
    L = ["", f"## Worse on seeds {cfg.get('seeds')}, not confirmed on seeds {cfg.get('confirm_seeds')} (treated as noise)", "",
         "| metric | map | baseline | first block | second block |", "|---|---|---|---|---|"]
    L += [f"| {m} | {k or '(all maps)'} | {b:.2f} | {v:.2f} | {v2:.2f} |" for m, k, b, v, v2 in dropped]
    return "\n".join(L) + "\n"


def fidelity_report(res, cfg):
    L = ["# Fidelity gate", "", f"baselines: {cfg.get('source', '?')} ({cfg.get('date', '?')})", ""]
    for title, key in (("Worse, NOT declared", "regress"), ("Worse, declared", "declared"), ("Better than baseline", "better")):
        L += [f"## {title} ({len(res[key])})", ""]
        if res[key]:
            L += ["| metric | map | baseline | now | dir |", "|---|---|---|---|---|"]
            for m, k, b, v in res[key]:
                L.append(f"| {m} | {k or '(all maps)'} | {b:.2f} | {v:.2f} | {cfg['metrics'][m].get('dir')} |")
        L.append("")
    return "\n".join(L) + "\n"


def card_fidelity_allow(tree):
    """`fidelity=` tokens of the running card's COMMITTED header (oracle_guard.card_declares_for finds the card)."""
    sys.path.insert(0, os.path.join(REPO, "scripts"))
    import oracle_guard
    cid, _ = oracle_guard.card_declares_for(os.environ, tree)
    if not cid:
        return set()
    toks = set()
    for part in oracle_guard.declares_of(oracle_guard.committed_card(cid)).split(";"):
        k, _, v = part.strip().partition("=")
        if k.strip() == "fidelity":
            toks |= {x for x in re.split(r"[,\s]+", v) if x}
    toks |= {x for x in re.split(r"[,\s]+", oracle_guard.approved(cid, "fidelity")) if x}   # gates.toml [approvals.<CARD>]
    return toks


def card_exposes(tree):
    """The `exposes` of the running card's approval ("" = an ordinary approval)."""
    sys.path.insert(0, os.path.join(REPO, "scripts"))
    import oracle_guard
    cid, _ = oracle_guard.card_declares_for(os.environ, tree)
    return oracle_guard.approved(cid, "exposes") if cid else ""


def exposed_filter(cfg, allow, exposes):
    """An approval marked `exposes = "<BUGCARD>"` says the fault is ALREADY ON MAIN. That has one mechanical consequence: main's
    baseline for a `down` metric on that map is above zero. A `metric@map` token that fails it is dropped (returns
    (kept, dropped)); `metric`, `@map` and `all` tokens are too wide to be an exposure and are dropped too."""
    if not exposes:
        return set(allow), []
    kept, void = set(), []
    for t in allow:
        m, _, k = t.partition("@")
        spec = cfg.get("metrics", {}).get(m)
        base = (spec or {}).get("maps", {}).get(k) if k and k != "agg" else None
        if spec and spec.get("dir") == "down" and base is not None and base > 0:
            kept.add(t)
        else:
            void.append(t)
    return kept, sorted(void)


def fidelity_toml(cfg):
    q = lambda x: (json.dumps(x) if isinstance(x, str) else str(x).lower() if isinstance(x, bool)
                   else repr(round(x, 4)) if isinstance(x, float) else str(x))
    L = ["# ---- fidelity (card FIDL; values written by gates.py --rebaseline-fidelity, directions/tolerances hand-set) ----",
         "[fidelity]"]
    for k in ("enabled", "seeds", "confirm_seeds", "ticks", "jobs", "source", "date"):
        if k in cfg:
            L.append(f"{k} = {q(cfg[k])}")
    for m, spec in cfg.get("metrics", {}).items():
        L += ["", f"[fidelity.metrics.{m}]"]
        L += [f"{k} = {q(v)}" for k, v in spec.items() if k != "maps"]
        L += [f"[fidelity.metrics.{m}.maps]"] + [f"{k} = {q(v)}" for k, v in sorted(spec.get("maps", {}).items())]
    return "\n".join(L) + "\n"


def fidelity_rebaseline(toml_path, new_json, only=None, source="", date=""):
    """Rewrite the [fidelity] block of gates.toml with the values of a fidelity_new.json (all metrics, or just `only`).
    Everything above the '# ---- fidelity' marker is kept byte for byte."""
    new = json.load(open(new_json))
    text = open(toml_path).read()
    cfg = tomllib.loads(text).get("fidelity", {})
    keep = text[:text.index("# ---- fidelity")] if "# ---- fidelity" in text else text + "\n"
    metrics = cfg.setdefault("metrics", {})
    for m, rec in new.items():
        if only and m not in only:
            continue
        spec = metrics.setdefault(m, dict(dir="track"))
        spec["agg"], spec["maps"] = rec["agg"], rec["maps"]
    if source:
        cfg["source"] = source
    if date:
        cfg["date"] = date
    open(toml_path, "w").write(keep + fidelity_toml(cfg))


def selftest_fidelity(a=None):
    """Fixtures: a regression is flagged, a declared move is MOVED, a better value passes, tolerances hold; the protect check
    on a throwaway repo; the toml round trip."""
    import tempfile
    bad = 0

    def chk(name, ok):
        nonlocal bad
        bad += not ok
        print(f"{'ok  ' if ok else 'FAIL'} {name}")
    spec = lambda d: dict(dir=d, tol_agg_abs=1.0, tol_map_abs=5.0, agg=10.0, maps={"m1": 10.0, "m2": 10.0})
    cfg = dict(metrics={"stuck": spec("down"), "contact": spec("up"), "free": dict(dir="track", agg=1.0, maps={"m1": 1.0})})
    new = lambda **kw: {k: dict(agg=v[0], maps=v[1]) for k, v in kw.items()}
    r = fidelity_compare(cfg, new(stuck=(10.5, {"m1": 10.0, "m2": 14.0})))
    chk("inside tolerance: nothing flagged", not r["regress"] and not r["declared"])
    r = fidelity_compare(cfg, new(stuck=(10.5, {"m1": 10.0, "m2": 16.0})))
    chk("one map past tol_map (down metric rose 6): flagged", r["regress"] == [("stuck", "m2", 10.0, 16.0)])
    r = fidelity_compare(cfg, new(stuck=(12.0, {"m1": 10.0, "m2": 10.0})))
    chk("agg past tol_agg: flagged with an empty map name", r["regress"] == [("stuck", "", 10.0, 12.0)])
    r = fidelity_compare(cfg, new(contact=(8.0, {"m1": 10.0, "m2": 10.0})))
    chk("an `up` metric that fell is a regression", [x[0] for x in r["regress"]] == ["contact"])
    r = fidelity_compare(cfg, new(stuck=(5.0, {"m1": 1.0, "m2": 10.0}), contact=(15.0, {"m1": 10.0, "m2": 10.0})))
    chk("better values are not regressions, and are listed", not r["regress"] and len(r["better"]) == 3)
    r = fidelity_compare(cfg, new(free=(50.0, {"m1": 50.0})))
    chk("a `track` metric never flags", not r["regress"] and not r["declared"] and r["checked"] == 0)
    bad_new = new(stuck=(12.0, {"m1": 10.0, "m2": 16.0}))
    for allow, want_reg, want_dec in ((set(), 2, 0), ({"stuck"}, 0, 2), ({"stuck@m2"}, 1, 1), ({"@m2"}, 1, 1), ({"stuck@agg"}, 1, 1), ({"all"}, 0, 2),
                                      ({"contact"}, 2, 0)):
        r = fidelity_compare(cfg, bad_new, allow)
        chk(f"declared {sorted(allow) or 'nothing'}: {want_reg} undeclared, {want_dec} declared", len(r["regress"]) == want_reg and len(r["declared"]) == want_dec)
    r = fidelity_compare(cfg, bad_new)           # confirmation: a per-map failure survives only if the 2nd block is worse too
    kept, dr = fidelity_confirm(cfg, r, new(stuck=(11.0, {"m1": 10.0, "m2": 11.0})))
    chk("confirm: m2 back inside tol on the 2nd block -> dropped as noise, agg stays", [(x[0], x[1]) for x in kept] == [("stuck", "")] and len(dr) == 1)
    kept, dr = fidelity_confirm(cfg, r, new(stuck=(12.0, {"m1": 10.0, "m2": 17.0})))
    chk("confirm: m2 worse on the 2nd block too -> kept", [(x[0], x[1]) for x in kept] == [("stuck", ""), ("stuck", "m2")] and not dr)
    kept, dr = fidelity_confirm(cfg, r, new(stuck=(12.0, {"m1": 10.0, "m2": 17.0})), new(stuck=(11.0, {"m1": 10.0, "m2": 14.0})))
    chk("confirm vs MAIN's own confirm block: m2 17 against main's 14 is inside tol -> dropped (main moved too)", [(x[0], x[1]) for x in kept] == [("stuck", "")] and len(dr) == 1)
    kept, dr = fidelity_confirm(cfg, r, new(stuck=(12.0, {"m1": 10.0, "m2": 17.0})), new(stuck=(10.0, {"m1": 10.0, "m2": 10.5})))
    chk("confirm vs MAIN's own confirm block: m2 17 against main's 10.5 -> kept", ("stuck", "m2") in [(x[0], x[1]) for x in kept] and not dr)
    kept, dr = fidelity_confirm(cfg, r, {})
    chk("confirm: no 2nd-block value -> the failure stands", len(kept) == 2 and not dr)
    # card FIDA: an all-map SUM gets the second look too, against main's own block-2 sum; an all-map mean never does
    sc = dict(metrics={"recycled": dict(dir="down", tol_agg_abs=12.0, tol_map_abs=10.0, agg=30.0, maps={"m1": 15.0, "m2": 15.0}),
                       "stuck": spec("down")})
    r = fidelity_compare(sc, new(recycled=(58.0, {"m1": 20.0, "m2": 20.0})))   # OBSV: 30 -> 58 on 1-8, 44 -> 49 on 9-16
    kept, dr = fidelity_confirm(sc, r, new(recycled=(49.0, {"m1": 25.0, "m2": 24.0})), new(recycled=(44.0, {"m1": 22.0, "m2": 22.0})))
    chk("FIDA: all-map sum worse on block 1, inside tol on block 2 (vs main's block 2) -> dropped as noise",
        not kept and dr == [("recycled", "", 30.0, 58.0, 49.0)] and "| recycled | (all maps) |" in fidelity_dropped_note(dr, sc))
    r = fidelity_compare(sc, new(recycled=(46.0, {"m1": 23.0, "m2": 23.0})))   # RLOOP: 30 -> 46 on 1-8, 44 -> 58 on 9-16
    kept, dr = fidelity_confirm(sc, r, new(recycled=(58.0, {"m1": 29.0, "m2": 29.0})), new(recycled=(44.0, {"m1": 22.0, "m2": 22.0})))
    chk("FIDA: all-map sum worse on both blocks -> kept", kept == [("recycled", "", 30.0, 46.0)] and not dr)
    kept, dr = fidelity_confirm(sc, r, {}, new(recycled=(44.0, {"m1": 22.0, "m2": 22.0})))
    chk("FIDA: no block-2 value -> the all-map failure stands", len(kept) == 1 and not dr)
    kept, dr = fidelity_confirm(sc, r, new(recycled=(40.0, {"m1": 20.0, "m2": 20.0})))
    chk("FIDA: no main block-2 sum -> the all-map failure stands (never against the seeds 1-8 baseline)", len(kept) == 1 and not dr)
    r = fidelity_compare(sc, new(stuck=(12.0, {"m1": 10.0, "m2": 10.0})))
    kept, dr = fidelity_confirm(sc, r, new(stuck=(10.0, {"m1": 10.0, "m2": 10.0})), new(stuck=(10.0, {"m1": 10.0, "m2": 10.0})))
    chk("FIDA: an all-map MEAN is never dropped", kept == [("stuck", "", 10.0, 12.0)] and not dr)
    # FIDSP: real sample parsing, including zero-count maps, across two seeds and the worker-pool path.
    import csv
    with tempfile.TemporaryDirectory() as t:
        bq = os.path.join(t, "botq")
        os.makedirs(bq)
        for seed in (1, 2):
            with open(os.path.join(t, f"sweep_s{seed}.txt"), "w") as f:
                f.write("".join(f"{m} team 1 wins at tick 100 stuck_max 0\n" for m in ("a", "b", "c")))
            for m, count, beh in (("a", 1, "recover"), ("b", 2, "recover"), ("c", 1, "travel")):
                with open(os.path.join(bq, f"{m}_s{seed}.csv"), "w", newline="") as f:
                    writer = csv.writer(f)
                    writer.writerow("t unit gen x y z beh veh human team target cp steer slide".split())
                    for unit in range(count):
                        for tick in range(81):  # Exactly 20 seconds, before bot_quality's warm-up cut.
                            writer.writerow((tick / 4, unit, 0, 0, 0, unit * 10, beh, -1, 0, 0, -1, 0, "clear", 0))
        measured = fidelity_measure(t)
        chk("FIDSP: loop20 sums seeds and maps, retaining a measured zero",
            measured.get("loop20") == dict(maps={"a": 2, "b": 4, "c": 0}, agg=6))
        cli = subprocess.check_output([sys.executable, os.path.join(REPO, "scripts/loop_spells.py"), bq], text=True)
        chk("FIDSP: hand CLI agrees with gate per-map and aggregate counts",
            "| a | 2 |" in cli and "| b | 4 |" in cli and "total spells 6," in cli)
        loop_cfg = dict(metrics={"loop20": dict(dir="down", agg=0, maps={"a": 0, "b": 0, "c": 0})})
        chk("FIDSP: a synthetic rise fails loop20@agg",
            ("loop20", "", 0, 6) in fidelity_compare(loop_cfg, measured)["regress"])
        equal_cfg = dict(metrics={"loop20": dict(dir="down", **measured["loop20"])})
        chk("FIDSP: unchanged loop counts pass", not fidelity_compare(equal_cfg, measured)["regress"])
    xc = dict(metrics={"stuck": dict(dir="down", maps={"m1": 0.0, "m2": 0.4}), "contact": dict(dir="up", maps={"m1": 5.0})})
    chk("exposed: main above zero keeps the token, main at zero voids it", exposed_filter(xc, {"stuck@m2", "stuck@m1"}, "BUG") == ({"stuck@m2"}, ["stuck@m1"]))
    chk("exposed: a whole-metric, whole-map, all, agg or `up` token is never an exposure",
        exposed_filter(xc, {"stuck", "@m2", "all", "stuck@agg", "contact@m1"}, "BUG")[0] == set())
    chk("an ordinary approval (no `exposes`) is untouched", exposed_filter(xc, {"stuck", "stuck@m1"}, "") == ({"stuck", "stuck@m1"}, []))
    with tempfile.TemporaryDirectory() as t:   # gates.toml [fidelity] round trip; the part above the marker is untouched
        tp = os.path.join(t, "g.toml")
        head = '[ci]\npassed  = 5\n\n'
        open(tp, "w").write(head)
        nj = os.path.join(t, "n.json")
        json.dump({"stuck": {"agg": 3.0, "maps": {"m1": 3.0}}}, open(nj, "w"))
        fidelity_rebaseline(tp, nj, None, "fixture", "2026-10-08")
        fidelity_rebaseline(tp, nj, {"stuck"}, "fixture2", "2026-10-09")
        txt = open(tp).read()
        d = tomllib.loads(txt)
        chk("rebaseline keeps the head and rewrites [fidelity] once", txt.startswith(head) and txt.count("[fidelity]") == 1
            and d["fidelity"]["metrics"]["stuck"]["maps"]["m1"] == 3.0 and d["fidelity"]["source"] == "fixture2")
    sys.path.insert(0, os.path.join(REPO, "scripts"))
    import oracle_guard
    with tempfile.TemporaryDirectory() as t:   # protect: a branch that changed a golden / gates.toml / nothing
        g = lambda *x: subprocess.run(["git", "-C", t, "-c", "user.name=t", "-c", "user.email=t@t", *x], capture_output=True, text=True)
        os.makedirs(os.path.join(t, "engine/goldens"))
        os.makedirs(os.path.join(t, "engine/artifacts/loop"))
        open(os.path.join(t, "engine/goldens/a.png"), "w").write("1")
        open(os.path.join(t, "engine/artifacts/loop/gates.toml"), "w").write("1")
        open(os.path.join(t, "x.rs"), "w").write("1")
        subprocess.run(["git", "init", "-q", "-b", "main", t])
        g("add", "-A"); g("commit", "-qm", "base")
        g("checkout", "-qb", "br")
        open(os.path.join(t, "x.rs"), "w").write("2"); g("commit", "-qam", "code")
        chk("protect: a code-only branch changes nothing protected", oracle_guard.changed_by_class(t, "main") == {})
        open(os.path.join(t, "engine/goldens/a.png"), "w").write("2")
        open(os.path.join(t, "engine/artifacts/loop/gates.toml"), "w").write("2"); g("commit", "-qam", "oracle")
        ch = oracle_guard.changed_by_class(t, "main")
        chk("protect: a golden and gates.toml are seen, by class", sorted(ch) == ["baselines", "goldens"])
        chk("protect: undeclared = FAIL set, declared (oracle=goldens,baselines) = empty",
            set(ch) - oracle_guard.oracle_declared("sweep") == {"baselines", "goldens"}
            and not set(ch) - oracle_guard.oracle_declared("oracle=goldens,baselines")
            and not set(ch) - oracle_guard.oracle_declared("oracle=any"))
    print("selftest-fidelity:", "FAILED" if bad else "all passed")
    return 1 if bad else 0


def touches_bake(files):
    return any(re.match(BAKE_CODE, f) for f in files)


def changed_files(tree, base):
    r = subprocess.run(["git", "-C", tree, "diff", "--name-only", f"{base}...HEAD"], capture_output=True, text=True)
    return r.stdout.split()


# What the DLC bake writes among the retail files: its heroes' bodies, its renamed textures, and the weapons a real bake
# lists in side_bf2/dlc_heroes.ron. The dot keeps a longer retail name out.
DLC_PREFIXES = ("side_bf2/dlc_sounds/", "side_bf2/effects/dlc/", "side_bf2/textures/dlch_",
                *(f"side_bf2/soldiers/{name}." for name in ("rep_inf_kitfisto", "cis_inf_asajjventress")),
                *(f"side_bf2/weapons/{name}." for name in ("bubblev3", "cis_inf_weap_sabre_b_chain", "cis_inf_weap_sabre_f_chain",
                                                           "cis_inf_weap_starblade", "fisto_l_fist", "fisto_l_foot", "fisto_r_fist",
                                                           "temp_saber")))


def bake_group_of(path):
    """The scripts/rebake.sh group that writes a baked file, from its path under assets_baked; None when no rule fits
    (gates_selftest.py holds the table against rebake.sh's group list)."""
    parts = path.split("/")
    top, sub = parts[0], parts[1] if len(parts) > 1 else ""
    if path in ("classes/bf2_dlc.ron", "side_bf2/dlc_heroes.ron") or path.startswith(DLC_PREFIXES):
        return "dlc"
    if sub == "sounds":  # every map dir's bank and core_bf*/sounds
        return "sounds"
    if top in ("tat1", "tat2"):
        return "load" if sub == "load" else top
    if re.match(r"bf[12]_", top):
        return "maps"  # the other maps' load screens are baked with the map
    if top in ("common_bf1", "common_bf2"):
        return "load" if sub == "load" else "common"
    if top in ("side_bf1", "side_bf2"):
        return "side"
    if top == "classes":
        return "classes"
    if top == "audio":
        return "global-sound" if sub.startswith("global_") else "beep"
    return None


def bake_verdict(cmp_txt, declared, full, exp_files):
    """(verdict, value, expected, stray) from cmp_bake.sh's output. `declared` = --bake-groups; `full` = every group was
    baked although only `declared` may move: a file that differs in another group is `stray` [(file, group)] and FAILS."""
    m = re.search(r"SAME=(\d+) DIFF=(\d+) ONLY_IN_A=(\d+) ONLY_IN_B=(\d+)", cmp_txt)
    same_, diff, a, b = map(int, m.groups())
    val = f"SAME={same_} DIFF={diff} ONLY_IN_A={a} ONLY_IN_B={b}"
    partial = bool(declared) and not full
    exp = "DIFF=0 ONLY_IN_A=0" + ("" if partial else f" ONLY_IN_B=0 ({exp_files} files)")
    ok = diff == 0 and a == 0 and (partial or b == 0)
    verdict = "FAIL" if same_ + diff + a == 0 else ("PASS" if ok else "MOVED")
    stray = []
    if full:
        for kind, f in re.findall(r"^(DIFF|ONLY_IN_A|ONLY_IN_B) (.+)$", cmp_txt, re.M):
            g = bake_group_of(f)
            if g not in declared:
                stray.append((f, g or "?"))
        if stray:
            groups = sorted({g for _, g in stray})
            first = "; ".join(f"{g}: {next(f for f, h in stray if h == g)}" for g in groups)
            known = [g for g in groups if g != "?"]
            val += (f" — {len(stray)} file(s) differ in group(s) the card does not declare ({first}; list in "
                    f"bake_undeclared.txt)" + (f" — declare rebake={','.join(known)} too" if known else ""))
            exp = f"no difference outside rebake={','.join(declared)}"
            verdict = "FAIL"
    return verdict, val, exp, stray


def auto_groups(tree, base):
    files = changed_files(tree, base)
    g = ["ci", "gpu", "map", "protect"]
    if any(re.match(r"engine/crates/(core|game|input|net)/", f) for f in files):
        g += ["sim", "sweep", "arena", "leak"]   # sweep also runs the fidelity rows (g_sweep): land.py only passes its own group list
    if touches_bake(files):
        g += ["bake"]
    if touches_bake(files) or any(re.match(r"engine/.*\.(rs|wgsl|toml|lock|json)$|scripts/(render_|version_|gameplay_|data/|world_fingerprint\.py|gates\.py)",f) for f in files):
        g += ["alignment"]
    if any(re.match(r"engine/crates/(render|game)/", f) for f in files):
        g += ["parity"]
    return g, files


STATE = "gates_state.json"  # in --out: the arguments, the finished groups and their rows, the running group and pids


def load_state(out):
    try:
        return json.load(open(os.path.join(out, STATE)))
    except (OSError, ValueError):
        return None


def save_json(path, obj):
    """Write-then-rename, so a kill mid-write never leaves half a state file."""
    tmp = f"{path}.{os.getpid()}.tmp"
    json.dump(obj, open(tmp, "w"), indent=1)
    os.replace(tmp, path)


def alive(pid, needle=""):
    """pid is a live process (and its command line holds `needle`, against pid reuse)."""
    try:
        os.kill(int(pid), 0)
    except PermissionError:
        pass
    except (OSError, TypeError, ValueError):
        return False
    try:
        return not needle or needle in open(f"/proc/{int(pid)}/cmdline", "rb").read().decode(errors="replace")
    except OSError:
        return False


# Effective run options shared by state keys and bare-resume restoration. None for fidelity_allow
# means use card declarations; an explicit empty string means allow no movements.
RUN_OPTIONS = dict(tree=None, target=None, bake_groups=[], keep_goldens=False, parity_allow="", golden_groups="auto",
                   base="main", leak_plan="always", fidelity_allow=None, fidelity_dir="", fidelity_confirm_dir="",
                   fidelity_both=False, alignment_reference="", alignment_cases=[])


def effective_options(a):
    return {k: (getattr(a, k, default) if k == "fidelity_allow" else getattr(a, k, default) or default)
            for k, default in RUN_OPTIONS.items()}


def restore_run_args(a, saved):
    """Restore known options; incomplete older provenance cannot match a new complete run key."""
    for k, v in effective_options(argparse.Namespace(**saved)).items():
        setattr(a, k, v)
    return saved["groups"]


def run_args(a, groups):
    """What a finished group's result depends on: a resume keeps it only when all of these are unchanged."""
    head = subprocess.run(["git", "-C", a.tree, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    toml = open(GATES_TOML, "rb").read() if os.path.exists(GATES_TOML) else b""
    return dict(effective_options(a), groups=groups, head=head, gates_toml=hashlib.sha1(toml).hexdigest())


def run(a, groups, resume=False, cls=None):
    """Run the groups, writing <out>/gates_state.json at every group start and end (and each child's pid). With
    resume, the groups a previous run of the SAME arguments finished are kept, not re-run; an unfinished one re-runs,
    and so does every group of a run that finished with a non-PASS row. Returns the rows."""
    global ON_CHILD
    G = (cls or Gates)(a)
    args = run_args(a, groups)
    old = load_state(a.out)
    done = []
    stale = os.path.join(a.out, "results.json")  # a crashed run must not leave the last run's rows for land.py to read
    if os.path.exists(stale):
        os.remove(stale)
    replay = old and old.get("status") == "finished" and old.get("exit") != 0  # a finished FAIL re-runs (a lost GPU, a fix)
    if resume and old and old.get("args") == args and not replay:
        done = list(old.get("done", []))
        G.rows = list(old.get("rows", []))
        G.moved_goldens = list(old.get("moved_goldens", []))
        print(f"[gates] --resume: kept {' '.join(done) or 'nothing'}; running {' '.join(g for g in groups if g not in done) or 'nothing'}", flush=True)
    elif resume and old:
        why = "finished with a failure (it re-runs whole)" if replay else \
            "other arguments, another HEAD or another gates.toml"
        print(f"[gates] --resume: the saved run had {why} — starting fresh", flush=True)
    st = dict(args=args, done=done, rows=list(G.rows), moved_goldens=list(G.moved_goldens), current=None, child=None,
              pid=os.getpid(), status="running", started=datetime.datetime.now().isoformat(timespec="seconds"))
    path = os.path.join(a.out, STATE)

    def child(pid):
        st.update(child=pid)
        save_json(path, st)
    ON_CHILD = child
    if "gpu" in groups and "gpu" not in done and not adapter_any(a) and G.golden_plan()[0]:
        present, line = adapter_check()
        print(f"[gates] {line}", flush=True)
        if not present:
            G.row("gpu", "vulkaninfo --summary", EXIT_ADAPTER_LOST, line, "the goldens' adapter present", ADAPTER_LOST)
            G.write()
            st.update(status="finished", exit=EXIT_ADAPTER_LOST, rows=list(G.rows),
                      ended=datetime.datetime.now().isoformat(timespec="seconds"))
            save_json(path, st)
            return G.rows
    G.asked = list(groups)   # g_sweep skips the fidelity rows on an identical sweep unless `fidelity` itself was asked for
    try:
        for g in groups:
            if g in done:
                continue
            st.update(current=g, child=None)
            save_json(path, st)
            getattr(G, "g_" + g)()
            done.append(g)
            st.update(done=done, rows=list(G.rows), moved_goldens=list(G.moved_goldens), current=None, child=None)
            save_json(path, st)
    finally:
        ON_CHILD = None
    G.write()
    st.update(status="finished", exit=rows_exit(G.rows),
              ended=datetime.datetime.now().isoformat(timespec="seconds"))
    save_json(path, st)
    return G.rows


def status_line(out):
    """One line and a code: 0 finished, 3 running, 4 dead before finishing (--resume it), 5 no state."""
    st = load_state(out)
    if not st:
        return f"gates {out}: no state", 5
    done = " ".join(st.get("done", [])) or "-"
    if st.get("status") == "finished":
        bad = [r["gate"] + "=" + r["verdict"] for r in st.get("rows", []) if r["verdict"] != "PASS"]
        return f"gates {out}: FINISHED {st.get('ended', '')} — {'every gate PASS' if not bad else ' '.join(bad)}", 0
    live = alive(st.get("pid"), "gates.py") or alive(st.get("pid"), "land.py")
    cur = st.get("current") or "-"
    ch = f", child {st['child']} {'alive' if alive(st['child']) else 'dead'}" if st.get("child") else ""
    if live:
        return f"gates {out}: RUNNING pid {st['pid']} — done [{done}], now {cur}{ch}", 3
    return f"gates {out}: DEAD pid {st['pid']} — done [{done}], died in {cur}{ch} — `gates.py --out {out} --resume`", 4


def detach(out, argv, log_name):
    """Re-run this command line in a new session (setsid; it outlives the caller), output to <out>/<log_name>; print
    the pid and return it. The caller polls --status."""
    os.makedirs(out, exist_ok=True)
    log = os.path.join(out, log_name)
    with open(log, "a") as f:
        f.write(f"\n=== detached {datetime.datetime.now():%Y-%m-%d %H:%M:%S}: {' '.join(argv)}\n")
        f.flush()
        p = subprocess.Popen([sys.executable, "-u", *argv], stdin=subprocess.DEVNULL, stdout=f, stderr=subprocess.STDOUT,
                             start_new_session=True, cwd=os.getcwd())  # the caller's relative --tree/--target/--out
    print(f"detached: pid {p.pid}, log {log}", flush=True)
    return p.pid


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out")
    ap.add_argument("--tree", default=REPO)
    ap.add_argument("--target")
    ap.add_argument("--groups", nargs="+", choices=ALL)
    ap.add_argument("--auto", action="store_true")
    ap.add_argument("--alignment-reference", help="independent PS2 assets for the explicit alignment gate")
    ap.add_argument("--alignment-case", dest="alignment_cases", action="append", metavar="NAME=ASSETS",
                    help="source bake for the alignment gate; repeat for each source")
    ap.add_argument("--base", default="main")
    ap.add_argument("--bake-groups", nargs="*")
    ap.add_argument("--keep-goldens", action="store_true", help="leave regenerated goldens in the tree (a landing commits them)")
    ap.add_argument("--golden-groups", default="auto", help="auto (engine/goldens/deps.toml + the diff vs --base), all, none, or a "
                    "comma list of tat1,hud,shell,spawn,gallery,walkabout")
    ap.add_argument("--leak-plan", choices=["always", "auto"], help="always (default): run the census when the leak group is asked "
                    "for; auto (--auto's default): skip it when no changed file can move it (engine/goldens/leak_deps.toml)")
    ap.add_argument("--parity-allow", help="maps the parity gate lets rise (the card's parity=<maps>, or all)")
    ap.add_argument("--fidelity-dir", help="reuse a finished `sweep_seeds.py --botq` out dir for the fidelity rows (no sweep run)")
    ap.add_argument("--fidelity-confirm-dir", help="with --fidelity-dir: a finished run of the confirm seeds for the second look "
                    "(no sweep run); without it a --fidelity-dir replay has no second look")
    ap.add_argument("--fidelity-allow", help="fidelity moves allowed (metric, metric@map, @map, all); default: the card's `fidelity=`")
    ap.add_argument("--rebaseline-fidelity", metavar="fidelity_new.json", help="write that run's values into gates.toml [fidelity] "
                    "(--only m1,m2 takes just those metrics; --source says where they came from) and exit")
    ap.add_argument("--only")
    ap.add_argument("--fidelity-both", action="store_true", help="a baseline run: also run the confirm seeds and write "
                    "fidelity/fidelity_confirm_new.json (--rebaseline-fidelity then installs it beside gates.toml's block)")
    ap.add_argument("--source", default="")
    ap.add_argument("--selftest-fidelity", action="store_true", help="the fidelity comparison on fixtures / --fidelity-dir runs")
    ap.add_argument("--detach", action="store_true", help="run in a new session (outlives the caller); poll --status")
    ap.add_argument("--adapter-any", action="store_true", help="render goldens on whatever GPU is there (no ADAPTER LOST "
                    "check; also OPENBF_ADAPTER_ANY=1)")
    ap.add_argument("--status", action="store_true", help="one line: finished / running / dead; exit 0 / 3 / 4 (5: no state)")
    ap.add_argument("--resume", action="store_true", help="keep the groups a dead run of the same arguments finished; "
                    "with --out alone, the arguments come from its state")
    a = ap.parse_args()
    if a.selftest_fidelity:
        sys.exit(selftest_fidelity(a))
    if a.rebaseline_fidelity:
        fidelity_rebaseline(GATES_TOML, a.rebaseline_fidelity, set(a.only.split(",")) if a.only else None, a.source,
                            datetime.date.today().isoformat())
        print(f"gates.toml [fidelity] rewritten from {a.rebaseline_fidelity}")
        cj = os.path.join(os.path.dirname(a.rebaseline_fidelity), "fidelity_confirm_new.json")
        if os.path.exists(cj) and not a.only:
            os.makedirs(os.path.dirname(CONFIRM_BASE), exist_ok=True)
            d = json.load(open(cj))
            d["source"] = a.source or cj
            json.dump(d, open(CONFIRM_BASE, "w"), indent=1, sort_keys=True)
            print(f"main's confirm-seed values ({d.get('seeds')}) written to {CONFIRM_BASE}")
        elif not a.only:
            print("NOTE: no fidelity_confirm_new.json beside it (run the baseline with --fidelity-both): a per-map failure is "
                  "confirmed against the seeds 1-8 baseline, which lets block-to-block noise through")
        return
    if not a.out:
        ap.error("--out is required")
    a.out = os.path.abspath(a.out)
    if a.status:
        line, code = status_line(a.out)
        print(line)
        sys.exit(code)
    st = load_state(a.out)
    if st and st.get("status") == "running" and alive(st.get("pid"), "gates.py"):
        sys.exit(f"a gates.py (pid {st['pid']}) is running on {a.out} — poll `--status`")
    if a.resume and st and st.get("status") == "running" and alive(st.get("child")):
        sys.exit(f"--resume: the dead run's child (pid {st['child']}) is still running and writing into {a.out} — "
                 f"wait for it or kill it first")
    if a.detach:
        detach(a.out, [os.path.abspath(__file__), *[x for x in sys.argv[1:] if x != "--detach"]], "gates.log")
        return
    if a.resume and not (a.groups or a.auto):
        if not st:
            sys.exit(f"--resume: no state in {a.out}")
        groups = restore_run_args(a, st["args"])
    else:
        a.tree = os.path.abspath(a.tree)
        a.target = os.path.abspath(a.target or os.path.join(a.tree, "engine/target"))
        if a.auto:
            a.leak_plan = a.leak_plan or "auto"
            groups, files = auto_groups(a.tree, a.base)
            print(f"[gates] --auto: {len(files)} files changed since {a.base} -> groups {' '.join(groups)}")
        else:
            groups = a.groups or ["ci", "gpu", "map", "alignment"]
        groups = [g for g in ALL if g in groups]
    rows = run(a, groups, a.resume)
    sys.exit(rows_exit(rows))


if __name__ == "__main__":
    main()
