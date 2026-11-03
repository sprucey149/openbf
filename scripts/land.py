#!/usr/bin/env python3
"""land.py — land a card's branch on main. Two phases, so a session LOOKS before anything is committed.

  scripts/land.py <ID> --pid <lane pid>            phase 1: rebase, gates on the branch, check every move is declared,
                                                   ff-merge, ci + rebake + goldens + behaviour gates on main; then STOPS
                                                   and prints what to VIEW. Nothing moved is committed yet.
  scripts/land.py <ID> --commit [--reject a,b]     phase 2: commit the declared golden moves (minus --reject), write a new
                                                   baseline + gates.toml when a behaviour move was declared, gpu gate on
                                                   main, remove the worktree/branch/<branch>_target, card -> done, journal line, claims released.
  scripts/land.py <ID> --abort                     undo phase 1's merge (revert commits, never reset), card -> blocked.
  scripts/land.py --self-test                      parse_declares over fixture strings (the rebake expansion and refusal), and
                                                   phase 1 killed at chosen steps / mid-gate, then --resume (all faked).

The card's `declares:` says what may move, e.g.  declares: goldens=maps/bf1_kas2.png,maps/bf2_kas2.png; rebake=maps; sweep
Keys: goldens=<paths under engine/goldens, or `any`>; rebake=<rebake.sh groups>; sweep; arena; sim (the sim hashes);
leak (the collision census, game/examples/leak_census.rs); parity=<maps, or `all`> (maps whose PS2 parity score may rise
past the gate's threshold — say why on the card). A card that declares goldens= also runs the parity gate on the branch.
`rebake=maps` also rebakes tat1 and tat2: they run the same `bake map` in groups of their own (card RBKM). A group
rebake.sh does not define is refused before anything runs (a typo would otherwise bake nothing and pass every gate).
ci counts may change freely (new tests) as long as nothing fails. Anything else that moves = refuse (exit 2) with the reason.
State: engine/artifacts/lead_tmp/<branch>/land/state.json, written after EVERY step (phase, step, the steps done, the
outputs so far, land.py's pid, the MAIN holder); every gate output under that directory (each gates.py run keeps its own
gates_state.json). So a session never has to die waiting on a landing (card LDET):
  scripts/land.py <ID> [--commit] --detach    start in a new session (setsid; outlives the caller), log <out>/land.log
  scripts/land.py <ID> --status               one line; exit 0 finished (phase 1 waiting for --commit, a dry run, landed, aborted),
                                              3 running, 4 dead before finishing, 5 nothing started, 6 refused (read the log)
  scripts/land.py <ID> --resume [--detach]    a dead phase 1 or 2 continues at its first unfinished step: finished steps are
                                              kept, a dead gates run re-runs only the groups it had not finished
A resume takes MAIN over from the claim the landing made itself (the lane pid); a fresh phase 1 over a merged one is refused.
ADAPTER LOST (card LOOPA): every run but --abort first asks, when the branch's diff would regenerate a golden, whether the GPU
the goldens were rendered on is there (gates.adapter_check); absent = one line and exit 7, nothing touched (no MAIN claim,
no rebase, the card as it was). --adapter-any (or OPENBF_ADAPTER_ANY=1) skips it, and is passed on to every gates run.
A gates run that loses the adapter mid-landing writes an ADAPTER LOST row, which reads as such, not as a move.
"""
import argparse, datetime, json, os, re, shutil, subprocess, sys, tomllib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import loopctl
import gates as gates_mod
from gates import auto_groups, restore_from_head

ROOT = loopctl.ROOT
GATES = os.path.join(ROOT, "scripts/gates.py")
GATES_TOML = os.path.join(ROOT, "engine/artifacts/loop/gates.toml")
LOCK = os.path.join(ROOT, "engine/artifacts/loop/ci.lock")
REBAKE = os.path.join(ROOT, "scripts/rebake.sh")
# The map bake runs in three groups (rebake.sh): `maps` is 30 of the ground maps, tat1/tat2 are the other two. A change
# to what `bake map` writes moves all three, and REFL's `rebake=maps` left the Tatooines' manifest.ron stale (RBKM).
REBAKE_EXPAND = {"maps": ["tat1", "tat2"]}


def rebake_groups():
    """rebake.sh's own group list (its REBAKE_GROUPS line), so the two cannot drift."""
    m = re.search(r'^REBAKE_GROUPS="([^"]*)"', open(REBAKE).read(), re.M)
    return m.group(1).split() if m else []


def git(*a, cwd=ROOT, check=True):
    r = subprocess.run(["git", *a], cwd=cwd, capture_output=True, text=True)
    if check and r.returncode:
        raise SystemExit(f"git {' '.join(a)} failed:\n{r.stdout}{r.stderr}")
    return r.stdout.strip()


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


def gates(tree, target, out, groups, extra=(), resume=False):
    cmd = [sys.executable, GATES, "--tree", tree, "--target", target, "--out", out, "--groups", *groups, *extra]
    res = os.path.join(out, "results.json")
    if os.path.exists(res):  # a gates.py that refuses or crashes must not leave an earlier attempt's rows to be judged
        os.remove(res)
    subprocess.run(cmd + (["--resume"] if resume else []))
    return json.load(open(os.path.join(out, "results.json")))


def rebase(wt):
    """Rebase the branch onto main; a rebase a killed run left half done is aborted first. (rc, output)"""
    subprocess.run(["git", "rebase", "--abort"], cwd=wt, capture_output=True)
    r = subprocess.run(["git", "rebase", "main"], cwd=wt, capture_output=True, text=True)
    if r.returncode:
        subprocess.run(["git", "rebase", "--abort"], cwd=wt)
    return r.returncode, r.stdout + r.stderr


def ff_merge(br):
    r = subprocess.run(["git", "merge", "--ff-only", br], cwd=ROOT, capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def rebake(groups, log):
    return subprocess.run(f"flock {LOCK} {ROOT}/scripts/rebake.sh {' '.join(groups)} > {log} 2>&1", shell=True).returncode


def write_summary(path, tree, out, rows):
    L = ["| gate | command | exit | value | expected | verdict |", "|---|---|---|---|---|---|"]
    for r in rows:
        L.append(f"| {r['gate']} | `{r['cmd']}` | {r['exit']} | {r['value']} | {r['expected']} | **{r['verdict']}** |")
    open(path, "w").write(f"Gates on `{tree}` (outputs in `{out}`)\n\n" + "\n".join(L) + "\n")


def golden_plan_args(out):
    """--golden-groups for a gates run on main: the groups the BRANCH run planned (there is no diff to judge by on main)."""
    try:
        plan = json.load(open(os.path.join(out, "branch", "goldens_plan.json")))["groups"]
    except (OSError, ValueError, KeyError):
        return []
    return ["--golden-groups", ",".join(plan) if plan else "none"]


def reuse_branch_gates(st, dec):
    """(ok, why). The gates on main would repeat the branch's on the same tree and the same baked assets, so their rows
    are the branch's — unless a rebake is declared (main's assets change), main's tree is not the one the branch
    gates ran on, or the every-10-landings full run is due."""
    if dec["rebake"]:
        return False, "a rebake is declared (main's baked assets change)"
    if not st.get("tree_branch"):
        return False, "no branch tree was recorded"
    if git("rev-parse", "HEAD^{tree}") != st["tree_branch"]:
        return False, "main's tree is not the tree the branch gates ran on"
    if gates_mod.full_due(ROOT, 10, gates_mod.MAIN_MARK):
        return False, "the full main gates are due (every 10 landings that touch engine/crates)"
    return True, "main's tree is the branch gates' tree"


def moved_list(quant):
    return [l.split("|")[1].strip() for l in open(quant).read().splitlines()[2:] if l.startswith("|")] if os.path.exists(quant) else []


# ---------- the state file: written after every step, so a dead landing resumes where it stopped (card LDET) ----------
LAND_TMP = os.path.join(ROOT, "engine/artifacts/lead_tmp")
STEPS1 = ["rebase", "gates_branch", "merge", "rebake", "gates_main"]


def paths(c):
    br = c.get("branch") or sys.exit(f"{c['id']}: card has no branch")
    return br, os.path.join(loopctl.WT_BASE, br), os.path.join(LAND_TMP, f"{br}_target"), os.path.join(LAND_TMP, br, "land")


def load_state(out):
    try:
        st = json.load(open(os.path.join(out, "state.json")))
    except (OSError, ValueError):
        return None
    if "phase" not in st and "rows_main" in st:  # written by the land.py before LDET: a finished phase 1
        st.update(phase=1, step="done", done=list(STEPS1), legacy=True)  # landed, aborted or waiting: it never said
    return st


def save(st):
    st["saved"] = datetime.datetime.now().isoformat(timespec="seconds")
    gates_mod.save_json(os.path.join(st["out"], "state.json"), st)


def take_main(pid, st):
    """Claim MAIN; a resume may also take it over from the claim this landing made itself (the lane pid that holds it
    is alive while the session that ran phase 1 is gone)."""
    holders = (str(pid),) + ((str(st["main_holder"]),) if st and st.get("main_holder") else ())
    return loopctl.claim("MAIN", pid, holders=holders)


def running(st):
    """Another land.py is working on this landing now."""
    return bool(st) and st.get("running") and st.get("pid") != os.getpid() and gates_mod.alive(st.get("pid"), "land.py")


def undeclared(rows, dec, moved_goldens):
    bad = []
    for r in rows:
        if r["verdict"] == gates_mod.ADAPTER_LOST:
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


def journal(line):
    open(os.path.join(loopctl.LOOP, "JOURNAL.md"), "a").write(line.rstrip() + "\n")


def phase2_problems(c, st, reject):
    """Recheck phase 1 evidence before any phase 2 mutation, including on resume.

    A group can emit several rows (and an enabled sweep also measures fidelity).
    Main either ran its own smaller plan or reused the complete branch plan.
    Rejected images may resolve golden movement, never a failed gate.
    """
    groups = st.get("groups")
    if not isinstance(groups, list) or not {"ci", "gpu", "map"} <= set(groups) or any(g not in gates_mod.ALL for g in groups):
        return ["missing phase 1 group plan"]
    if any(step not in st.get("done", []) for step in STEPS1):
        return ["missing phase 1 completed steps"]
    dec = parse_declares(c.get("declares"))
    moved = st.get("moved_goldens")
    if not isinstance(moved, list):
        return ["missing phase 1 golden inventory"]
    # Include current paths as well as saved ones: a changed receipt must not conceal a new move.
    current = [l[3:].strip() for l in git("status", "--porcelain", "--", "engine/goldens").splitlines() if l.strip()]
    accepted = sorted((set(moved) | set(current)) - reject)
    required_main = ["ci", "gpu"] + [g for g in ("sim", "sweep", "arena", "leak") if g in groups]
    if dec["rebake"]:
        required_main.append("bake")
    main_groups = st.get("main_groups", required_main)
    if not isinstance(main_groups, list) or not set(required_main) <= set(main_groups) or any(g not in gates_mod.ALL for g in main_groups):
        return ["missing/malformed main gate plan"]
    with open(GATES_TOML, "rb") as f:
        cfg = tomllib.load(f).get("fidelity")
    family = {"map": ["map", "protect"], "sim": ["sim", "simhash"],
              "arena": ["arena_flat", "arena_tat2"], "leak": ["ordnance", "leak"]}
    if cfg and cfg.get("enabled", True):
        family["sweep"] = ["sweep", "fidelity"]
    bad = []
    if set(current) - set(moved):
        bad.append("current golden moves missing from phase 1 inventory")
    for name, plan in (("branch", groups), ("main", main_groups)):
        rows = st.get("rows_" + name)
        if not isinstance(rows, list) or not rows or any(not isinstance(r, dict) or
                not {"gate", "verdict", "value"} <= r.keys() for r in rows):
            bad.append(f"missing/malformed phase 1 {name} results")
            continue
        expected = {r for g in plan for r in family.get(g, [g])}
        missing = expected - {r["gate"] for r in rows}
        if missing:
            bad.append(f"missing phase 1 {name} rows: {', '.join(sorted(missing))}")
        bad += [f"{name}: {r['gate']} has unknown verdict {r['verdict']}" for r in rows
                if r["verdict"] not in ("PASS", "MOVED", "FAIL", gates_mod.ADAPTER_LOST)]
        bad += [f"{name}: {p}" for p in undeclared(rows, dec, accepted)]
        if name == "main":
            bad += [f"bake on main not reproducible: {r['value']}" for r in rows if r["gate"] == "bake" and r["verdict"] != "PASS"]
            if not moved and any(r["gate"] == "gpu" and r["verdict"] == "MOVED" for r in rows):
                bad.append("missing phase 1 moved golden paths")
    # Check paths even if a stale saved gpu row incorrectly says PASS.
    bad += undeclared([dict(gate="gpu", verdict="MOVED", value="golden inventory")], dec, accepted)
    return bad


def phase1(c, pid, dry=False, resume=False):
    br, wt, target, out = paths(c)
    os.makedirs(out, exist_ok=True)
    dec = parse_declares(c.get("declares"))
    unknown = [g for g in dec["rebake"] if g not in rebake_groups()]
    if unknown:
        sys.exit(f"{c['id']}: declares rebake={','.join(unknown)}, which scripts/rebake.sh does not define "
                 f"(groups: {' '.join(rebake_groups())}) — fix the card's `declares:`")
    st = load_state(out)
    open_ = st and st.get("phase") == 1 and not st.get("closed") and not (st.get("legacy") and not resume)
    if resume:
        if not open_:
            sys.exit(f"{c['id']}: no unfinished phase 1 to resume in {out} (`--status`)")
        if st["step"] == "done":
            print(f"{c['id']}: phase 1 already finished {st.get('time', '')} — not re-run. VIEW the sheets, then --commit / --abort.")
            return
        print(f"{c['id']}: resuming phase 1 — finished: {' '.join(st['done']) or 'nothing'}; next: "
              f"{next(s_ for s_ in STEPS1 if s_ not in st['done'])}", flush=True)
    else:
        if open_ and "merge" in st["done"] + [st.get("step")]:
            sys.exit(f"{c['id']}: a phase 1 already merged ({st['step']}) — `--resume`, `--commit` or `--abort`, never a fresh phase 1")
        st = dict(id=c["id"], branch=br, worktree=wt, out=out, declares=c.get("declares"), phase=1, step=None, done=[],
                  started=datetime.datetime.now().isoformat(timespec="seconds"))
    if not take_main(pid, st):
        sys.exit("MAIN is held by a live session — land later (take another job)")
    st.update(pid=os.getpid(), running=True, main_holder=str(pid))
    st.pop("refused", None)
    done = st["done"]
    was = st.get("step")  # the step a dead run was in: its gates resume, keeping the groups it finished

    def begin(name, **kw):
        st.update(step=name, **kw)
        save(st)

    def finish(name, **kw):
        done.append(name)
        st.update(kw)
        save(st)

    def stop(msg, code, step=None):
        st.update(running=False, **({"step": step} if step else {}))
        save(st)
        loopctl.release("MAIN", pid)
        print(msg)
        sys.exit(code)

    save(st)
    if "merge" not in done and git("status", "--porcelain", "--untracked-files=no"):
        stop("main checkout is dirty (tracked files) — refusing", 1)
    # 1. rebase
    if "rebase" not in done:
        begin("rebase")
        rc, txt = rebase(wt)
        if rc:
            stop(f"rebase onto main conflicts — resolve inside the card's files in {wt}, or block the card:\n{txt}", 1, "refused")
        finish("rebase")
    # 2. gates on the rebased branch (auto groups + everything the card declares)
    if "groups" not in st:
        groups = set(auto_groups(wt, "main")[0])
        if dec["sweep"] or dec["arena"] or dec["sim"] or dec["leak"]:
            groups |= {"sim", "sweep", "arena", "leak"}
        if dec["rebake"]:
            groups |= {"bake"}
        if dec["goldens"] or dec["parity"]:
            groups |= {"parity"}
        st["groups"] = landing_gate_groups(groups)
    order = st["groups"]
    if "gates_branch" not in done:
        begin("gates_branch")
        extra = ["--bake-groups", *dec["rebake"]] if dec["rebake"] else []
        extra += ["--parity-allow", ",".join(dec["parity"])] if dec["parity"] else []
        extra += [] if dec["leak"] else ["--leak-plan", "auto"]  # a card that declares leak runs the census; the rest only when it can move (GSPD)
        loopctl.seed_target(target)  # a parked card's target was removed at parking
        rows_b = gates(wt, target, os.path.join(out, "branch"), order, extra, resume=resume and was == "gates_branch")
        bad = undeclared(rows_b, dec, moved_list(os.path.join(out, "branch", "goldens", "QUANT.md")))
        if any(r["verdict"] == gates_mod.ADAPTER_LOST for r in rows_b):
            stop("\n" + "\n  ".join(bad) + f"\nNothing was merged; `land.py {c['id']} --resume` once the GPU is back.",
                 gates_mod.EXIT_ADAPTER_LOST, "refused")
        if bad:
            stop("\nNOT LANDABLE — every move must be declared on the card (`declares:`):\n  " + "\n  ".join(bad) +
                 f"\nOutputs: {out}/branch. Fix on the branch, or declare the move (if it is the card's intent), or block the card.",
                 2, "refused")
        finish("gates_branch", rows_branch=rows_b, tree_branch=git("rev-parse", "HEAD^{tree}", cwd=wt))
    if dry:
        stop(f"\nDRY RUN: branch gates pass the declared-move check; stopping before the merge. Outputs: {out}/branch", 0, "dry-run")
    # 3. fast-forward main (`before` is saved first: a run killed inside the merge is recognised by HEAD == the branch)
    if "merge" not in done:
        tip = git("rev-parse", br)
        if was == "merge" and st.get("before") and git("rev-parse", "HEAD") == tip:
            finish("merge", after=tip)
        else:
            begin("merge", before=git("rev-parse", "HEAD"))
            rc, txt = ff_merge(br)
            if rc:
                stop(f"fast-forward refused (main moved during the gates?) — rerun `land.py {c['id']}`:\n{txt}", 1, "refused")
            finish("merge", after=git("rev-parse", "HEAD"))
    # 4. on main: rebake, then ci, goldens (kept for commit), behaviour gates
    main_groups = ["ci", "gpu"] + [g for g in ("sim", "sweep", "arena", "leak") if g in order]
    if "rebake" not in done:
        if dec["rebake"]:
            begin("rebake")
            if rebake(dec["rebake"], f"{out}/rebake_main.txt"):
                st.update(running=False)
                save(st)
                print(f"rebake on main FAILED (see {out}/rebake_main.txt) — run `land.py {c['id']} --abort`"); sys.exit(3)
        finish("rebake")
    if dec["rebake"]:
        main_groups.append("bake")
    if "gates_main" not in done:
        begin("gates_main")
        reuse, why = reuse_branch_gates(st, dec)
        mo = os.path.join(out, "main")
        gg = golden_plan_args(out)
        if reuse:
            # the tree and the assets are the branch gates' own: copy their outputs and re-run only what must be redone on
            # main — the golden regeneration whose PNGs a landing commits (the branch run restored them from HEAD)
            shutil.copytree(os.path.join(out, "branch"), mo, dirs_exist_ok=True, ignore=shutil.ignore_patterns("gates_state.json", "gates.log"))
            by = {r["gate"]: r for r in st["rows_branch"]}
            todo = ["gpu"] if by.get("gpu", {}).get("verdict") == "MOVED" else []
            print(f"[land] gates on main: reusing the branch rows ({why}); re-running: {' '.join(todo) or 'nothing'}", flush=True)
            rows_m = [dict(r) for r in st["rows_branch"]]
            if todo:
                new_rows = {r["gate"]: r for r in gates(ROOT, os.path.join(ROOT, "engine/target"), mo, todo, ["--keep-goldens", *gg],
                                                      resume=resume and was == "gates_main")}
                rows_m = [new_rows.get(r["gate"], r) for r in rows_m]
            write_summary(os.path.join(mo, "SUMMARY.md"), ROOT, mo, rows_m)
            json.dump(rows_m, open(os.path.join(mo, "results.json"), "w"), indent=1)
        else:
            print(f"[land] gates on main: running all of them ({why})", flush=True)
            rows_m = gates(ROOT, os.path.join(ROOT, "engine/target"), mo, main_groups,
                           ["--keep-goldens", *gg, *(["--bake-groups", *dec["rebake"]] if dec["rebake"] else [])],
                           resume=resume and was == "gates_main")
            if all(r["verdict"] != "FAIL" for r in rows_m):
                gates_mod.mark_full(ROOT, gates_mod.MAIN_MARK)
        # on main the bake gate compares the scratch bake with the freshly rebaked tree: it must be DIFF=0
        moved_m = moved_list(os.path.join(out, "main", "goldens", "QUANT.md"))
        bad = undeclared([r for r in rows_m if r["gate"] != "bake"], dec, moved_m)
        bad += [f"bake on main not reproducible: {r['value']}" for r in rows_m if r["gate"] == "bake" and r["verdict"] != "PASS"]
        finish("gates_main", rows_main=rows_m, main_groups=order if reuse else main_groups, moved_goldens=moved_m, problems=bad)
    st.update(step="done", running=False, time=datetime.datetime.now().isoformat(timespec="seconds"))
    save(st)
    before, after, bad, moved_m = st["before"], st["after"], st["problems"], st["moved_goldens"]
    print(f"\n=== PHASE 1 DONE: main fast-forwarded {before[:8]}..{after[:8]} ({br}); nothing committed beyond the branch.")
    if bad:
        print("PROBLEMS on main:\n  " + "\n  ".join(bad) + f"\nIf they are real: `land.py {c['id']} --abort`.")
    if moved_m:
        print(f"VIEW every sheet before phase 2: {out}/main/goldens/pair_*.png  (numbers: {out}/main/goldens/QUANT.md)")
    for g in ("sweep", "arena_flat", "arena_tat2", "sim", "simhash", "leak"):
        for r in st["rows_main"]:
            if r["gate"] == g and r["verdict"] == "MOVED":
                print(f"{g} moved on main (declared): outputs in {out}/main — check them; phase 2 re-baselines.")
    print(f"Then: scripts/land.py {c['id']} --commit [--reject <golden,...>]")


def phase2(c, reject, pid, resume=False):
    br, _, _, out = paths(c)
    st = load_state(out)
    if not st or st.get("closed") or not (st.get("phase") == 2 or st.get("step") == "done"):
        sys.exit(f"{c['id']}: phase 1 has not finished (`land.py {c['id']} --status`)")
    if st.get("phase") == 2 and st.get("step") == "landed":
        print(f"{c['id']}: already landed {st.get('time', '')}"); return
    if st.get("phase") == 2:
        if not resume:
            sys.exit(f"{c['id']}: a phase 2 started ({st['step']}) — `--resume` it")
        reject = st["reject"]
        print(f"{c['id']}: resuming phase 2 — finished: {' '.join(st['done2']) or 'nothing'}", flush=True)
    if not take_main(pid, st):
        sys.exit("MAIN is held by a live session — commit later")
    rej = {r if r.startswith("engine/goldens/") else "engine/goldens/" + r for r in reject}
    bad = phase2_problems(c, st, rej)
    if bad:
        sys.exit("NOT LANDABLE — unresolved phase 1 results:\n  " + "\n  ".join(bad))
    if st.get("phase") != 2:
        st.update(phase=2, step=None, done2=[], reject=reject)
        st.pop("legacy", None)  # from here on it is a state this land.py writes
    st.update(pid=os.getpid(), running=True, main_holder=str(pid))
    st.pop("refused", None)
    done, was = st["done2"], st.get("step")

    def begin(name):
        st.update(step=name)
        save(st)

    def finish(name, **kw):
        done.append(name)
        st.update(kw)
        save(st)

    head = git("rev-parse", "HEAD")
    if head not in (st["after"], st.get("golden_commit")):
        # killed between the golden commit and recording it: that commit is ours
        if was == "goldens" and git("rev-parse", "HEAD~1") == st["after"] and \
                git("log", "-1", "--format=%s").startswith(f"Goldens: {c['id']} "):
            st.update(golden_commit=head)
        else:
            st.update(running=False); save(st)
            sys.exit("main moved since phase 1 — re-run phase 1")
    # goldens: commit the accepted moves, restore the rejected ones
    keep = [m for m in st["moved_goldens"] if m not in rej]
    if "goldens" not in done:
        begin("goldens")
        for m in st["moved_goldens"]:
            if m in rej:
                restore_from_head(ROOT, m)
        if keep and not st.get("golden_commit"):
            q = open(os.path.join(out, "main", "goldens", "QUANT.md")).read()
            git("add", "--", *keep)
            git("commit", "-m", f"Goldens: {c['id']} — {len(keep)} declared move(s), viewed before commit\n\n{q}")
            st.update(golden_commit=git("rev-parse", "HEAD"))
        finish("goldens")
    commits = [st["golden_commit"][:7]] if st.get("golden_commit") else []
    # behaviour re-baseline
    rows = {r["gate"]: r for r in st["rows_main"]}
    moved = [k for k in ("sweep", "arena_flat", "arena_tat2", "sim", "simhash", "leak") if rows.get(k, {}).get("verdict") == "MOVED"]
    if "toml" not in done:
        begin("toml")
        rebaseline(c, st, out, rows, moved)
        finish("toml")
    # the gpu gate on main must now be clean
    if "gpu_after" not in done:
        begin("gpu_after")
        if keep:
            rows_after = gates(ROOT, os.path.join(ROOT, "engine/target"), os.path.join(out, "main_after"), ["gpu"],
                               golden_plan_args(out), resume=resume and was == "gpu_after")
            if rows_after[0]["verdict"] != "PASS":
                st.update(running=False); save(st)
                print(f"gpu gate on main is not clean after the commit: {rows_after[0]['value']} — investigate before anything else"); sys.exit(4)
        finish("gpu_after")
    # clean up the worktree and branch
    if "cleanup" not in done:
        begin("cleanup")
        wt = st["worktree"]
        for link in ("engine/assets_baked", "engine/artifacts", "extracted", "PC_swbf2_exe", "community_mods"):
            p = os.path.join(wt, link)
            if os.path.islink(p):
                os.remove(p)
        git("worktree", "remove", wt, check=False)
        git("branch", "-d", br, check=False)
        # and its cargo target (TGTC): ~20 GB exclusive each; phase 1 re-seeds one by reflink if a card ever lands again
        freed = loopctl.remove_target(os.path.join(LAND_TMP, f"{br}_target"), quiet=True)
        finish("cleanup", target_freed=freed)
    begin("landed")
    sha = git("rev-parse", "--short", "HEAD")
    subprocess.run([sys.executable, os.path.join(ROOT, "scripts/loopctl.py"), "landed", c["id"], st["after"][:8]])
    table = open(os.path.join(out, "main", "SUMMARY.md")).read()
    open(os.path.join(out, "GATES_FOR_REPORT.md"), "w").write(
        f"## Gates — branch\n{open(os.path.join(out, 'branch', 'SUMMARY.md')).read()}\n## Gates — main\n{table}\n"
        f"Goldens committed: {', '.join(keep) or 'none'}; rejected: {', '.join(sorted(rej)) or 'none'}; re-baselined: {', '.join(moved) or 'nothing'}\n")
    journal(f"- LANDED {c['id']} as {st['after'][:8]} (+{', '.join(commits) or 'no golden commit'}): gates {out}/GATES_FOR_REPORT.md; "
            f"goldens {len(keep)}; re-baselined {', '.join(moved) or 'nothing'}; target {'removed' if st.get('target_freed') else 'kept (absent or busy)'}")
    loopctl.release("MAIN", pid); loopctl.release(c["id"])
    done.append("landed")
    st.update(running=False, closed="landed", time=datetime.datetime.now().isoformat(timespec="seconds"))
    save(st)
    print(f"LANDED {c['id']}: main {sha}. Paste {out}/GATES_FOR_REPORT.md into your report.")


def rebaseline(c, st, out, rows, moved):
    """gates.toml: the ci counts, and a new baseline dir + hashes when a behaviour gate moved (declared)."""
    g = tomllib.load(open(GATES_TOML, "rb"))
    ci = rows.get("ci", {}).get("value", "")
    text = open(GATES_TOML).read()
    if "toml_before" not in st:  # --abort puts it back
        st["toml_before"] = text
        save(st)
    m = re.match(r"(\d+) / (\d+) / (\d+)", ci)
    if m:
        text = re.sub(r"passed  = \d+", f"passed  = {m.group(1)}", text)
        text = re.sub(r"ignored = \d+", f"ignored = {m.group(3)}", text)
    if moved:
        nb = os.path.join(LAND_TMP, "loop_baselines", f"{datetime.datetime.now():%Y%m%d_%H%M}_{c['id']}")
        os.makedirs(nb, exist_ok=True)
        mo = os.path.join(out, "main")
        for src, dst in (("sweep_1.txt", "sweep_main_1.txt"), ("sweep_1.err", "sweep_main_1.err"), ("arena_flat_1.txt", "arena_flat_1.txt"),
                         ("arena_tat2_1.txt", "arena_tat2_1.txt"), ("sim_gates.txt", "sim_gates.txt"), ("sim_hash_100.txt", "sim_hash_100.txt"),
                         ("leak_1.txt", "leak_main_1.txt")):
            s_ = os.path.join(mo, src)
            if os.path.exists(s_):
                shutil.copy(s_, os.path.join(nb, dst))
            elif os.path.exists(os.path.join(g["baseline_dir"], dst)):
                shutil.copy(os.path.join(g["baseline_dir"], dst), os.path.join(nb, dst))
        open(os.path.join(nb, "SUMMARY.txt"), "w").write(f"{c['id']} re-baseline on main {st['after'][:8]} — moved: {', '.join(moved)}\n"
                                                         f"declares: {c.get('declares')}\nprevious: {g['baseline_dir']}\n")
        text = re.sub(r'baseline_dir = ".*?"', f'baseline_dir = "{nb}"', text)
        if "sweep" in moved and os.path.exists(os.path.join(mo, "sweep_1.txt")):
            foot = [l for l in open(os.path.join(mo, "sweep_1.txt")).read().splitlines() if l.startswith("sweep:")]
            if foot:
                text = re.sub(r'footer = ".*?"', f'footer = "{foot[-1]}"', text)
        if "sim" in moved or "simhash" in moved:
            sg = open(os.path.join(mo, "sim_gates.txt")).read()
            for ticks, key in (("407", "walk"), ("120", "aim"), ("600", "hash600")):
                mm = re.search(rf"sim hash after {ticks} ticks: ([0-9a-f]{{16}})", sg)
                if mm:
                    text = re.sub(rf'{key}\s*= ".*?"', f'{key} = "{mm.group(1)}"', text)
            h = open(os.path.join(mo, "sim_hash_100.txt")).read().split()
            if h:
                text = re.sub(r'hash100\s*= ".*?"', f'hash100   = "{h[-1]}"', text)
        if "sim" in moved:
            mb = rebase_motion([mo, os.path.join(out, "branch")], nb, g.get("motion_base", ""))
            if mb:
                text = re.sub(r'motion_base\s*= ".*?"', f'motion_base  = "{mb}"', text)
    open(GATES_TOML, "w").write(text)
    # a declared OR APPROVED fidelity move: the landed branch's numbers become the new [fidelity] baseline (only those metrics).
    # Approvals live in gates.toml [approvals.<CARD>] (cards are not in git): BSPN landed on 2026-10-12 with the baseline still
    # at the main before it, and the next landing would have been blamed for BSPN's approved move.
    tokens = set()
    for part in (c.get("declares") or "").split(";"):
        k, _, v = part.strip().partition("=")
        if k.strip() == "fidelity":
            tokens |= {x for x in re.split(r"[,\s]+", v) if x}
    try:
        ap = tomllib.load(open(GATES_TOML, "rb")).get("approvals", {}).get(c["id"], {})
        tokens |= {x for x in re.split(r"[,\s]+", str(ap.get("fidelity", ""))) if x}
    except (OSError, ValueError):
        pass
    fj = os.path.join(out, "main", "fidelity", "fidelity_new.json")
    if tokens and os.path.exists(fj):
        only = None if ("all" in tokens or any(t.startswith("@") for t in tokens)) else {t.partition("@")[0] for t in tokens}
        gates_mod.fidelity_rebaseline(GATES_TOML, fj, only=only, source=f"{c['id']} landing, main {st['after'][:8]}",
                                      date=f"{datetime.date.today()}")
        journal(f"- FIDELITY baseline re-seeded by {c['id']}'s landing for {', '.join(sorted(only)) if only else 'every metric'} "
                f"(seeds 1-8, main {st['after'][:8]}); the confirm block is older - planner: re-take with --fidelity-both")


def rebase_motion(runs, nb, old_base):
    """The sim gate's motion.csv tree for a new baseline dir: each of walk/aim from the first run dir that has it
    (main's, else the branch's reused rows), else from the old base. Returns the new motion_base, or None when a file
    is missing everywhere. (STHOP moved the walk trajectory and only the hashes were re-baselined: every later landing
    read `motion DIFFERS` against the old tree — cleanse CLNC.)"""
    found = {}
    for m in ("walk", "aim"):
        for d in [os.path.join(r, "sim", "motion") for r in runs] + [old_base]:
            f = os.path.join(d, m, "motion.csv")
            if d and os.path.exists(f):
                found[m] = f
                break
        else:
            return None
    for m, f in found.items():
        os.makedirs(os.path.join(nb, "motion", m), exist_ok=True)
        shutil.copy(f, os.path.join(nb, "motion", m, "motion.csv"))
    return os.path.join(nb, "motion")


def abort(c, pid):
    br, _, _, out = paths(c)
    st = load_state(out)
    if not st or st.get("closed"):
        sys.exit(f"{c['id']}: no open landing to abort in {out}")
    for m in st.get("moved_goldens", []):
        restore_from_head(ROOT, m)
    before = st.get("before")
    after = st.get("after") or (git("rev-parse", "HEAD") if before and git("rev-parse", "HEAD") == git("rev-parse", br, check=False) else before)
    head = git("rev-parse", "HEAD")
    tip = head if head in (after, st.get("golden_commit")) else None  # phase 2 may have committed goldens on top
    reverted = bool(before and tip and tip != before)
    if reverted:
        git("revert", "--no-edit", f"{before}..{tip}")
    if st.get("toml_before") is not None:
        open(GATES_TOML, "w").write(st["toml_before"])
    subprocess.run([sys.executable, os.path.join(ROOT, "scripts/loopctl.py"), "set", c["id"], "status=blocked",
                    f"resume=landing aborted {datetime.date.today()}: see {out}/state.json problems"])
    what = f"reverted {before[:8]}..{tip[:8]}" if reverted else \
        ("nothing merged" if before == after else f"NOT reverted: main moved past {(after or '-')[:8]} — revert by hand")
    journal(f"- LANDING ABORTED {c['id']}: {what}; card blocked; {out}/state.json")
    st.update(closed="aborted", running=False)
    save(st)
    mh = loopctl.holder(os.path.join(loopctl.LOCKS, "MAIN"))
    if mh in (str(pid), str(st.get("main_holder")), str(os.getpid())) or not loopctl.held(mh):  # never another lane's MAIN
        loopctl.release("MAIN", mh)
    print(f"aborted: {what} (history kept); card blocked")


def status_line(c):
    """One line and a code: 0 finished (phase 1 waiting for --commit, a dry run, landed, aborted), 3 running, 4 dead before
    finishing (`--resume`), 5 nothing started, 6 stopped by a refusal (read the log)."""
    br, _, _, out = paths(c)
    st = load_state(out)
    if not st:
        return f"{c['id']}: no landing state in {out}", 5
    mh = loopctl.holder(os.path.join(loopctl.LOCKS, "MAIN"))
    main = f"MAIN {mh} ({'alive' if loopctl.held(mh) else 'dead'})" if mh != "0" else "MAIN free"
    ph, step = st.get("phase"), st.get("step")
    done = " ".join(st.get("done2" if ph == 2 else "done", [])) or "-"
    sub = {"gates_branch": "branch", "gates_main": "main", "gpu_after": "main_after"}.get(step)
    gl = f" | {gates_mod.status_line(os.path.join(out, sub))[0]}" if sub else ""
    head = f"{c['id']}: phase {ph} step {step} (done: {done})"
    if st.get("closed"):
        return f"{c['id']}: {st['closed'].upper()} {st.get('time', st.get('saved', ''))}", 0
    if st.get("running") and gates_mod.alive(st.get("pid"), "land.py"):
        return f"{head} — RUNNING pid {st['pid']}{gl} | {main}", 3
    if st.get("refused"):
        return f"{c['id']}: REFUSED {st.get('refused_at', '')}: {st['refused'].splitlines()[0]} | {main}", 6
    if st.get("legacy"):
        return (f"{c['id']}: phase 1 finished {st.get('time', '')} by the pre-LDET land.py (it never recorded a commit or an "
                f"abort); card status {c.get('status')} | {main}"), 0
    if ph == 1 and step == "done":
        return (f"{c['id']}: PHASE 1 DONE {st.get('time', '')} — {len(st.get('moved_goldens', []))} golden(s) moved, "
                f"{len(st.get('problems', []))} problem(s); VIEW {out}/main/goldens, then --commit | {main}"), 0
    if step == "dry-run":
        return f"{head} — DRY RUN finished; branch outputs in {out}/branch | {main}", 0
    if step == "refused":
        return f"{head} — STOPPED ({step}); see {out}/land.log or the caller's output | {main}", 6
    return f"{head} — DEAD pid {st.get('pid')}{gl} | {main} — `land.py {c['id']} --resume`", 4


def self_test():
    cases = [  # (declares, expected rebake list)
        ("rebake=maps", ["maps", "tat1", "tat2"]),
        ("goldens=maps/a.png; rebake=side,maps; sweep", ["side", "maps", "tat1", "tat2"]),
        ("rebake=tat1,tat2,maps", ["tat1", "tat2", "maps"]),
        ("rebake=beep", ["beep"]),
        ("sweep", []),
    ]
    bad = 0
    for s, want in (("goldens=maps/a.png; parity=bf1_hot1, bf2_kas2", ["bf1_hot1", "bf2_kas2"]), ("parity=all", ["all"]), ("sweep", [])):
        got = parse_declares(s)["parity"]
        bad += got != want
        print(f"{'ok  ' if got == want else 'FAIL'} {s!r} -> parity {got} (want {want})")
    for s, want in cases:
        got = parse_declares(s)["rebake"]
        bad += got != want
        print(f"{'ok  ' if got == want else 'FAIL'} {s!r} -> rebake {got} (want {want})")
    known = rebake_groups()
    for g, want in (("maps", True), ("tat1", True), ("global-sound", True), ("map", False), ("<the", False)):
        ok = (g in known) == want
        bad += not ok
        print(f"{'ok  ' if ok else 'FAIL'} rebake group {g!r} {'known' if want else 'refused'}")
    import tempfile
    with tempfile.TemporaryDirectory() as t:  # rebase_motion: main's walk, the branch's aim, then the old base
        def put(*parts, text="x"):
            os.makedirs(os.path.join(t, *parts[:-1]), exist_ok=True)
            open(os.path.join(t, *parts), "w").write(text)
        put("main", "sim", "motion", "walk", "motion.csv", text="main-walk")
        put("branch", "sim", "motion", "walk", "motion.csv", text="branch-walk")
        put("branch", "sim", "motion", "aim", "motion.csv", text="branch-aim")
        put("old", "aim", "motion.csv", text="old-aim")
        runs = [os.path.join(t, "main"), os.path.join(t, "branch")]
        mb = rebase_motion(runs, os.path.join(t, "nb"), os.path.join(t, "old"))
        got = [open(os.path.join(mb, m, "motion.csv")).read() for m in ("walk", "aim")] if mb else None
        ok = mb == os.path.join(t, "nb", "motion") and got == ["main-walk", "branch-aim"]
        bad += not ok
        print(f"{'ok  ' if ok else 'FAIL'} rebase_motion takes main's walk and the branch's aim -> {got}")
        mb = rebase_motion([os.path.join(t, "none")], os.path.join(t, "nb2"), os.path.join(t, "old"))
        ok = mb is None and not os.path.exists(os.path.join(t, "nb2"))
        bad += not ok
        print(f"{'ok  ' if ok else 'FAIL'} rebase_motion with no walk anywhere re-points nothing")
    bad += resume_self_test()
    print("self-test: " + ("PASS" if not bad else f"{bad} FAILED"))
    return 1 if bad else 0


class Killed(BaseException):
    """The self-test's stand-in for a session dying mid-step (nothing catches it, like SIGKILL)."""


def resume_self_test():
    """Phase 1 killed at chosen points, then --resume: which steps and which gate groups run again. Everything outside
    this module is faked (git, rebake, MAIN, the gate groups); the state files live in a temp dir."""
    import tempfile, types
    g_ = globals()
    saved = {k: g_[k] for k in ("git", "rebase", "rebake", "ff_merge", "gates", "auto_groups", "LAND_TMP")}
    saved_l = {k: getattr(loopctl, k) for k in ("claim", "release", "holder", "seed_target")}
    calls, kill, repo, verdicts = [], set(), {}, {}
    saved_g = {k: getattr(gates_mod, k) for k in ("full_due", "mark_full")}

    class FakeGates(gates_mod.Gates):
        def _g(s, name):
            where = os.path.basename(s.out)
            calls.append(f"{where}:{name}")
            if f"{where}:{name}" in kill:
                kill.discard(f"{where}:{name}")
                raise Killed()
            s.row(name, name, 0, "ok", "ok", verdicts.get(f"{where}:{name}", "PASS"))
    for n in gates_mod.ALL:
        setattr(FakeGates, "g_" + n, lambda s, n=n: s._g(n))

    def fake_git(*a, cwd=ROOT, check=True):
        if a[:1] == ("rev-parse",):
            return repo["head"] if a[1] == "HEAD" else "tip"
        if a[:2] == ("merge", "--ff-only"):
            calls.append("merge")
            repo["head"] = "tip"
        return ""

    def fake_gates(tree, target, out, groups, extra=(), resume=False):
        where = os.path.basename(out)
        if f"{where}:start" in kill:
            kill.discard(f"{where}:start")
            raise Killed()
        ns = types.SimpleNamespace(tree=tree, target=target, out=out, bake_groups=[], keep_goldens=False)
        return gates_mod.run(ns, groups, resume, FakeGates)

    bad = 0
    try:
        g_.update(git=fake_git, rebase=lambda wt: (calls.append("rebase"), (0, ""))[1],
                  rebake=lambda gr, log: calls.append("rebake") or 0, gates=fake_gates,
                  ff_merge=lambda br: (fake_git("merge", "--ff-only"), (0, ""))[1],
                  auto_groups=lambda wt, base: (["ci", "gpu", "map"], []))
        loopctl.claim = lambda n, p, **kw: True
        loopctl.release = lambda n, p=None: None
        loopctl.holder = lambda d: "0"
        loopctl.seed_target = lambda p: None
        with tempfile.TemporaryDirectory() as tmp:
            g_["LAND_TMP"] = tmp
            card = dict(id="TEST", branch="t", declares="rebake=maps")
            for name, kills, want in (
                ("killed after the merge, before the main gates: resumes at the main gates",
                 {"main:start"}, ["main:ci", "main:gpu", "main:bake"]),
                ("killed mid-gpu-gate on the branch: only that gate (and the ones after it) re-run",
                 {"branch:gpu"}, ["branch:gpu", "branch:bake", "branch:map", "merge", "rebake", "main:ci", "main:gpu", "main:bake"]),
                ("killed mid-gpu-gate on main: ci is kept, gpu re-runs",
                 {"main:gpu"}, ["main:gpu", "main:bake"]),
            ):
                shutil.rmtree(os.path.join(tmp, "t"), ignore_errors=True)
                repo["head"] = "base"
                calls.clear(); kill.clear(); kill.update(kills)
                try:
                    phase1(card, 1)
                    got_kill = False
                except Killed:
                    got_kill = True
                first = list(calls)
                calls.clear()
                phase1(card, 1, resume=True)
                ok = got_kill and calls == want and load_state(os.path.join(tmp, "t", "land"))["step"] == "done"
                bad += not ok
                print(f"{'ok  ' if ok else 'FAIL'} {name}\n       first run {first}\n       resume    {calls}" + ("" if ok else f"\n       want      {want}"))
                if name.startswith("killed mid-gpu-gate on main"):
                    calls.clear()
                    phase1(card, 1, resume=True)
                    ok = calls == []
                    bad += not ok
                    print(f"{'ok  ' if ok else 'FAIL'} a finished phase 1 is not re-run by --resume (ran {calls})")
                    try:
                        phase1(card, 1)
                        ok = False
                    except SystemExit:
                        ok = True
                    bad += not ok
                    print(f"{'ok  ' if ok else 'FAIL'} a fresh phase 1 over a merged one is refused")
                    line, code = status_line(card)
                    ok = code == 0 and "PHASE 1 DONE" in line
                    bad += not ok
                    print(f"{'ok  ' if ok else 'FAIL'} --status: {line}")
            # no rebake declared and the same tree on main: the main gates reuse the branch's rows
            gates_mod.full_due, gates_mod.mark_full = (lambda *a, **k: False), (lambda *a, **k: None)
            card2 = dict(id="TEST2", branch="t2", declares="")
            for name, vd, want_main in (
                ("same tree, branch goldens unchanged: no gate re-runs on main", {}, []),
                ("same tree, branch goldens MOVED: only gpu re-runs on main (its PNGs are kept for the commit)", {"branch:gpu": "MOVED"}, ["main:gpu"]),
            ):
                shutil.rmtree(os.path.join(tmp, "t2"), ignore_errors=True)
                repo["head"] = "base"
                calls.clear(); kill.clear(); verdicts.clear(); verdicts.update(vd)
                phase1(card2, 1)
                got = [c for c in calls if c.startswith("main:")]
                st2 = load_state(os.path.join(tmp, "t2", "land"))
                ok = got == want_main and st2["step"] == "done" and [r["gate"] for r in st2["rows_main"]] == ["ci", "gpu", "map"] \
                    and os.path.exists(os.path.join(tmp, "t2", "land", "main", "SUMMARY.md"))
                bad += not ok
                print(f"{'ok  ' if ok else 'FAIL'} {name}\n       main calls {got} (want {want_main})")
            verdicts.clear()
            gates_mod.full_due = lambda *a, **k: True   # the every-10 backstop: everything runs on main again
            shutil.rmtree(os.path.join(tmp, "t2"), ignore_errors=True)
            repo["head"] = "base"; calls.clear()
            phase1(card2, 1)
            got = [c for c in calls if c.startswith("main:")]
            ok = got == ["main:ci", "main:gpu"]
            bad += not ok
            print(f"{'ok  ' if ok else 'FAIL'} the every-10 full run is due: ci and gpu re-run on main ({got})")
            st = load_state(os.path.join(tmp, "t", "land"))
            st.update(step="gates_main", done=[d for d in st["done"] if d != "gates_main"], running=True, pid=99999999)
            save(st)
            line, code = status_line(card)
            ok = code == 4 and "DEAD" in line
            bad += not ok
            print(f"{'ok  ' if ok else 'FAIL'} --status of a dead run: {line}")
            record_refusal(st["out"], "MAIN is held by a live session — commit later")
            line, code = status_line(card)
            ok = code == 6 and "MAIN is held" in line
            bad += not ok
            print(f"{'ok  ' if ok else 'FAIL'} a refusal reaches --status: {line}")
            loopctl.claim = lambda n, p, holders=(): "6991" in holders
            ok = take_main(6991, {}) and not take_main(4242, {})
            bad += not ok
            print(f"{'ok  ' if ok else 'FAIL'} MAIN held by the asking lane pid is taken; held by another live pid is refused")
    finally:
        g_.update(saved)
        for k, v in saved_l.items():
            setattr(loopctl, k, v)
        for k, v in saved_g.items():
            setattr(gates_mod, k, v)
    return bad


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("id", nargs="?")
    ap.add_argument("--pid", default=os.environ.get("OPENBF_LANE_PID", str(os.getppid())))
    ap.add_argument("--commit", action="store_true")
    ap.add_argument("--abort", action="store_true")
    ap.add_argument("--reject", default="")
    ap.add_argument("--dry-run", action="store_true", help="phase 1 up to the merge, then stop (tests / a pre-check)")
    ap.add_argument("--detach", action="store_true", help="run in a new session (outlives the caller), log <out>/land.log; poll --status")
    ap.add_argument("--status", action="store_true", help="one line; exit 0 finished, 3 running, 4 dead (--resume), 5 none, 6 refused (why)")
    ap.add_argument("--resume", action="store_true", help="continue a dead phase 1 or phase 2 at its first unfinished step")
    ap.add_argument("--adapter-any", action="store_true", help="render goldens on whatever GPU is there (no ADAPTER LOST check)")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        sys.exit(self_test())
    if not a.id:
        ap.error("the card ID is required")
    cs = loopctl.cards()
    if a.id not in cs:
        sys.exit(f"no card {a.id}")
    c = cs[a.id]
    if a.status:
        line, code = status_line(c)
        print(line)
        sys.exit(code)
    out = paths(c)[3]
    st = load_state(out)
    if running(st):
        sys.exit(f"a land.py (pid {st['pid']}) is working on {a.id} now — poll `land.py {a.id} --status`")
    # A NEW landing does not start under STOP (the third session to launch one after seeing it, 2026-10-09 / 10-11: a check in
    # the same shell line as the launch does not stop the launch). A landing already under way is finished with --resume or --commit, or taken back with --abort, as before.
    if os.path.exists(os.path.join(ROOT, "STOP")) and not (a.resume or a.abort or a.commit or a.reject or a.dry_run):
        sys.exit(f"STOP is present: no new landing starts. {a.id} stays parked; the next loop start lands it.")
    if a.adapter_any:
        os.environ["OPENBF_ADAPTER_ANY"] = "1"  # every gates.py this landing runs inherits it
    if not a.abort and not gates_mod.adapter_any():
        lost = adapter_lost(c, st, a.commit or a.reject)
        if lost:
            print(f"{a.id}: {lost}\nNothing touched: no MAIN claim, no rebase; the card is as it was.")
            sys.exit(gates_mod.EXIT_ADAPTER_LOST)
    if a.detach:
        # the lane pid resolved HERE: the detached child's parent is gone, and MAIN must carry the lane's pid
        argv, skip = [os.path.abspath(__file__)], False
        for x in sys.argv[1:]:
            if not skip and x != "--detach" and not x.startswith("--pid"):
                argv.append(x)
            skip = x == "--pid"
        argv += ["--pid", a.pid]
        gates_mod.detach(out, argv, "land.log")
        print(f"poll: scripts/land.py {a.id} --status   (exit 0 finished, 3 running, 4 dead -> --resume)")
        return
    try:
        if a.abort:
            abort(c, a.pid)
        elif a.commit or (a.resume and st and st.get("phase") == 2):
            phase2(c, [x for x in a.reject.split(",") if x.strip()], a.pid, a.resume)
        else:
            phase1(c, a.pid, a.dry_run, a.resume)
    except SystemExit as e:
        if not (a.abort or a.commit or a.resume) and st and st.get("phase") == 1 and st.get("step") == "done":
            raise  # a stray fresh run refused over a finished phase 1: --status keeps saying "VIEW, then --commit"
        record_refusal(out, e.code)
        raise


def adapter_lost(c, st, commit=False):
    """The ADAPTER LOST line when this run would regenerate a golden and the goldens' GPU is absent, else None. Phase 2
    renders only when phase 1 moved a golden; past the merge the branch run's plan says (main has no diff to judge by);
    before it, the branch's diff against main (gates.plan_goldens)."""
    br = c.get("branch")
    wt = os.path.join(loopctl.WT_BASE, br) if br else None
    if commit or (st and st.get("phase") == 2):
        need = bool(st and st.get("moved_goldens"))
    elif st and st.get("phase") == 1 and not st.get("closed") and "merge" in st.get("done", []):
        need = golden_plan_args(st.get("out") or paths(c)[3]) != ["--golden-groups", "none"]  # no plan: main plans all
    else:
        need = bool(wt and os.path.isdir(wt) and gates_mod.plan_goldens(wt, "main")[0])
    if not need:
        return None
    present, line = gates_mod.adapter_check()
    return None if present else line


def record_refusal(out, msg):
    """A refusal with a message (MAIN held, main moved, ...) goes into the state too: a detached run's only other
    trace is its log, and --status would keep showing the state from before."""
    st = load_state(out)
    if isinstance(msg, str) and st and not st.get("closed"):
        st.update(refused=msg, refused_at=datetime.datetime.now().isoformat(timespec="seconds"), running=False)
        save(st)


if __name__ == "__main__":
    main()
