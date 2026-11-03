#!/usr/bin/env python3
"""parity.py — how close the port is to the PS2: re-render the port at every stored PCSX2 pose and score each pair (card PARITY).

  scripts/parity.py run [--maps a,b] [--tree T --target D] [--out DIR] [--bake DIR] [--publish] [--keep-renders]
  scripts/parity.py main                     # the main branch's scores (cached per sha + bake; rendered and published when missing)
  scripts/parity.py gate --tree T --target D --out DIR [--allow a,b] [--base CSV] [--threshold L]   # the gates.py `parity` gate
  scripts/parity.py compare <base.csv> <new.csv> [--maps a,b] [--allow a,b]
  scripts/parity.py scoreboard               # one line from loop/parity/

Poses: engine/artifacts/pcsx2/ref/<map>/*.json and ref/<map>/walk/*.json (grab.py / walk_bf*.py), each beside its PS2
frame <spot>.png. A map without a bake (engine/assets_baked/<map>) is skipped and listed. `run` builds the tree's
`game` example `pose_render` (engine/crates/game/examples/pose_render.rs), renders every pose into <out>/render/ —
never into ref/*/port/ — holding ci.lock then gpu.lock (the order gates.py uses), and scores:
  block   mean over the unmasked 16x16 blocks of |block mean(PS2) - block mean(port)|, averaged over R,G,B (0..255):
          colour and brightness at the scale of objects, blind to texel phase and a pixel of misregistration
  dluma   mean luma (Rec.601) of the port minus the PS2's, unmasked pixels: + = the port is brighter
  p99     the 99th-percentile luma of each frame and their difference (dp99 = port - PS2): highlights, shine, sky
Masks (both frames, fixed per game, 640x480 coordinates, a block touching one is dropped): the HUD (counters, ammo,
reticle, weapon panel, hint line), the minimap and the player's own soldier; the port draws neither soldier nor HUD.
Outputs in <out>: scores.csv (map, spot, bearing, block, dluma, p99_ps2, p99_port, dp99, eye_error_m), summary.md
(per-map means, the worst posts) and worst.png (the 12 worst pairs, PS2 | port). The renders (~325 MB a full run) are
deleted once scored unless --keep-renders. --publish (only a full run of main's HEAD) copies them to
engine/artifacts/loop/parity/ latest.csv + summary.md + worst.png and appends history.csv (date, main sha, pairs,
all-maps mean block error, per-map means); `main` does that whenever it has to score a new main HEAD, and keeps each
sha/bake's scores in loop/parity/main/<sha>_<bake>.csv.

`gate` scores the tree (all maps: a full run is ~30 s of rendering) and the `main` branch's commit (`main`; built in
../openbf-wt/parity-main with its own target when the main checkout is on another commit), then `compare`s them:
per map, the mean block error over the (map, spot) pairs both scored, tree - main; a map FAILS when it rises by more
than --threshold (0.5 levels) unless --allow names it (the card's `declares: parity=<maps>`, or `all`). Exit 3: main
has no pose_render yet (no baseline). Deterministic: the same binary over the same bake scores byte-identically. The
tree renders the shared bake, so a bake-only change shows only once rebaked on main (like the goldens).
"""
import argparse, csv, datetime, glob, hashlib, json, os, re, subprocess, sys, time
from contextlib import contextmanager

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
ART = os.path.join(REPO, "engine/artifacts")
REF = os.path.join(ART, "pcsx2/ref")
PUB = os.path.join(ART, "loop/parity")
LOCK = os.path.join(ART, "loop/ci.lock")
GPU_LOCK = os.path.join(ART, "loop/gpu.lock")
BAKED = os.path.join(REPO, "engine/assets_baked")


@contextmanager
def ci_lock():
    """Lock before building, reading the bake identity, rendering or publishing.

    Callers already holding it pass locked=True to avoid flocking a second fd.
    Lock order is ci.lock, main.lock, then gpu.lock throughout this module.
    """
    import fcntl
    with open(LOCK, "a") as lk:
        fcntl.flock(lk, fcntl.LOCK_EX)
        yield


def bake_fingerprint():
    """Content identity of the whole shared bake: every file's relative path and bytes.

    Not manifests only: common/side/classes rebakes write no manifest.ron, and pose_render draws COMMON.
    Read only under ci.lock, which rebakes also hold. Legacy SHA-only scores are
    deliberately ignored. A missing bake must not create a reusable empty key.
    """
    if not glob.glob(os.path.join(BAKED, "**/manifest.ron"), recursive=True):
        sys.exit(f"parity: no bake manifests under {BAKED}")
    digest = hashlib.sha256()
    for root, dirs, files in os.walk(BAKED):
        dirs.sort()
        for f in sorted(files):
            path = os.path.join(root, f)
            name = os.path.relpath(path, BAKED).replace(os.sep, "/").encode()
            digest.update(len(name).to_bytes(8, "big"))
            digest.update(name)
            with open(path, "rb") as fh:
                digest.update(os.fstat(fh.fileno()).st_size.to_bytes(8, "big"))
                for chunk in iter(lambda: fh.read(1 << 20), b""):
                    digest.update(chunk)
    return digest.hexdigest()


def cache_path(head, bake):
    return os.path.join(PUB, "main", f"{head}_{bake}.csv")


def main_root():
    """The main checkout (a worktree's scripts/ must not score itself as main): the git common dir's parent."""
    g = subprocess.run(["git", "-C", REPO, "rev-parse", "--path-format=absolute", "--git-common-dir"],
                       capture_output=True, text=True).stdout.strip()
    return os.path.dirname(g) if g else REPO
W, H, B = 640, 480, 16
THRESHOLD = 0.5
COLS = ["map", "spot", "bearing", "block", "dluma", "p99_ps2", "p99_port", "dp99", "eye_error_m"]

# (x0, y0, x1, y1), exclusive ends, on the 640x480 PS2 frame; read off walk and PXC frames of both games (2026-09-30)
MASKS = {
    "bf1": [(185, 10, 455, 55),     # the two tickets counters and their bars
            (0, 25, 175, 90),       # ammo / grenades
            (465, 25, 640, 90),     # the health bar and stance icon
            (270, 130, 370, 220),   # the reticle
            (20, 300, 190, 470),    # the minimap
            (200, 140, 440, 480)],  # the player's own soldier (chase camera)
    "bf2": [(130, 35, 215, 70),     # the friendly reinforcement count
            (400, 35, 500, 70),     # the enemy's
            (50, 195, 120, 265),    # the kill / capture tallies
            (480, 10, 625, 145),    # the minimap
            (260, 100, 380, 210),   # the reticle
            (0, 280, 270, 480),     # the weapon panel, health and stamina
            (0, 430, 520, 480),     # the hint line
            (230, 250, 420, 480)],  # the player's own soldier
}


_MASKS = {}


def masks():
    import numpy as np
    if _MASKS:
        return _MASKS
    out = _MASKS
    for g, rects in MASKS.items():
        m = np.zeros((H, W), bool)
        for x0, y0, x1, y1 in rects:
            m[y0:y1, x0:x1] = True
        blk = m.reshape(H // B, B, W // B, B).any(axis=(1, 3))
        out[g] = (m, blk)
    return out


def poses(maps=None):
    """[(map, spot, json path, ps2 png)] for every stored pose with its PS2 frame; and the skipped maps."""
    rows, skipped = [], []
    for d in sorted(os.listdir(REF)):
        if not os.path.isdir(os.path.join(REF, d)) or d == "states" or (maps and d not in maps):
            continue
        if not os.path.isdir(os.path.join(BAKED, d)):
            skipped.append(d)
            continue
        for j in sorted(glob.glob(os.path.join(REF, d, "*.json")) + glob.glob(os.path.join(REF, d, "walk", "*.json"))):
            png = j[:-5] + ".png"
            if os.path.exists(png):
                rows.append((d, os.path.relpath(j[:-5], os.path.join(REF, d)), j, png))
    return rows, skipped


def render(tree, target, out, rows, bake_root):
    """Build and render every pose. Caller holds ci.lock; renderer also takes gpu.lock."""
    env = dict(os.environ, CARGO_TARGET_DIR=target)
    r = subprocess.run(["cargo", "build", "--release", "-p", "game", "--example", "pose_render"],
                       cwd=os.path.join(tree, "engine"), env=env, capture_output=True, text=True)
    if r.returncode:
        sys.exit(f"pose_render build failed:\n{r.stderr[-3000:]}")
    exe = os.path.join(target, "release/examples/pose_render")
    by_map = {}
    for m, spot, j, _ in rows:
        p = json.load(open(j))
        dst = os.path.join(out, "render", m, spot + ".png")
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        x, y, z = p["eye"]
        by_map.setdefault(m, []).append(f"{dst}={x},{y},{z},{p['yaw_deg']},{p['pitch_deg']},{p['fov_v_deg']}")
    # OPENBF_TEXTURE_TIER=1: the PS2's own textures, as port.py renders the pairs
    renv = dict(os.environ, OPENBF_TEXTURE_TIER="1")
    script = "set -e\n" + "\n".join(
        " ".join(q(a) for a in [exe, os.path.join(bake_root, m)] + specs) + " > /dev/null" for m, specs in by_map.items())
    sh = os.path.join(out, "render.sh")  # one line per map: too long for an argv
    open(sh, "w").write(script + "\n")
    t0 = time.time()
    # Shared, as gates.py and pcsx2_session.py take it (measured 2026-10-11): exclusive, a gate waited for every
    # PCSX2 session to close (card PLOCK).
    r = subprocess.run(["flock", "-s", GPU_LOCK, "bash", sh], env=renv, cwd=os.path.join(tree, "engine"))
    if r.returncode:
        sys.exit(f"pose_render failed (exit {r.returncode})")
    return time.time() - t0


def q(a):
    return "'" + a.replace("'", "'\\''") + "'"


def luma(a):
    return a[..., 0] * 0.299 + a[..., 1] * 0.587 + a[..., 2] * 0.114


def score(ps2_png, port_png, game, M):
    import numpy as np
    from PIL import Image
    def load(p):
        im = Image.open(p).convert("RGB")
        return np.asarray(im if im.size == (W, H) else im.resize((W, H), Image.LANCZOS), dtype=np.float64)
    a, b = load(ps2_png), load(port_png)
    m, blk = M[game]
    ba = a.reshape(H // B, B, W // B, B, 3).mean(axis=(1, 3))
    bb = b.reshape(H // B, B, W // B, B, 3).mean(axis=(1, 3))
    block = np.abs(ba - bb).mean(axis=2)[~blk].mean()
    la, lb = luma(a)[~m], luma(b)[~m]
    pa, pb = np.percentile(la, 99), np.percentile(lb, 99)
    return block, lb.mean() - la.mean(), pa, pb, pb - pa


def score_row(arg):
    out, (m, spot, j, png) = arg
    port = os.path.join(out, "render", m, spot + ".png")
    if not os.path.exists(port):
        return None
    p = json.load(open(j))
    bearing = spot.rsplit("_b", 1)[1] if "_b" in spot and spot.rsplit("_b", 1)[1].isdigit() else ""
    s = score(png, port, p.get("game") or m.split("_")[0], masks())
    return dict(zip(COLS, [m, spot, bearing, *(round(float(v), 3) for v in s), round(float(p.get("eye_error_m", 0)), 3)]))


def score_all(out, rows):
    from multiprocessing import Pool
    with Pool(min(16, os.cpu_count() or 1)) as pool:
        res = [r for r in pool.map(score_row, [(out, r) for r in rows], chunksize=8) if r]
    with open(os.path.join(out, "scores.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, COLS)
        w.writeheader()
        w.writerows(res)
    return res


def read_csv(p):
    return [dict(r, block=float(r["block"]), dluma=float(r["dluma"]), dp99=float(r["dp99"])) for r in csv.DictReader(open(p))]


def per_map(res):
    d = {}
    for r in res:
        d.setdefault(r["map"], []).append(r)
    return {m: dict(n=len(v), block=sum(r["block"] for r in v) / len(v), dluma=sum(r["dluma"] for r in v) / len(v),
                    dp99=sum(r["dp99"] for r in v) / len(v)) for m, v in sorted(d.items())}


def post_of(spot):
    """The post a frame stands at: the first two tokens of its name (walk/cp0_cp2_b090 and cp0_cp2_beachhead_v1 -> cp0_cp2),
    so a post's walkabout bearings and PXC views pool (187 posts, 4-12 frames each, 2026-09-30); a `postA_v1`-style name
    pools on its first token (postA_spawn, postA_v1, postA_v3_up -> postA)."""
    t = spot.split("/")[-1].split("_")
    return t[0] if re.fullmatch(r"post[A-Z]", t[0]) else "_".join(t[:2])


def worst_posts(res, n=5):
    """The n posts with the highest MEDIAN block error over their frames: one frame a bystander, an explosion or a
    lens flare fills (the PS2 draws them, the pose render cannot) does not rank a post."""
    import statistics
    d = {}
    for r in res:
        d.setdefault((r["map"], post_of(r["spot"])), []).append(r["block"])
    return sorted(((statistics.median(v), k) for k, v in d.items()), reverse=True)[:n]


def summary(res, skipped, head, secs, out):
    pm = per_map(res)
    allm = sum(r["block"] for r in res) / max(1, len(res))
    L = [f"# Parity {datetime.datetime.now():%Y-%m-%d %H:%M} — tree HEAD {head[:10]}", "",
         f"{len(res)} pairs over {len(pm)} maps; mean block error **{allm:.2f}** (0..255, lower is closer to the PS2); "
         f"render {secs:.0f} s." + (f" No bake, skipped: {', '.join(skipped)}." if skipped else ""), "",
         "| map | pairs | block | dluma (port-PS2) | dp99 |", "|---|---|---|---|---|"]
    L += [f"| {m} | {v['n']} | {v['block']:.2f} | {v['dluma']:+.1f} | {v['dp99']:+.1f} |" for m, v in pm.items()]
    L += ["", "Worst posts (median block error over their frames; worst.png shows the worst single pairs — a pair's score also "
          "holds what the PS2 draws and the pose render cannot: soldiers, effects, flares):", ""]
    L += [f"- {m} {p}: {e:.2f}" for e, (m, p) in worst_posts(res, 10)]
    open(os.path.join(out, "summary.md"), "w").write("\n".join(L) + "\n")
    return pm, allm


def git_head(tree):
    return git_rev(tree, "HEAD")


def git_rev(tree, ref):
    return subprocess.run(["git", "-C", tree, "rev-parse", "--verify", "-q", ref + "^{commit}"],
                          capture_output=True, text=True).stdout.strip()


def publish(out, pm, allm, head, bake):
    import shutil
    os.makedirs(os.path.join(PUB, "main"), exist_ok=True)
    for f, g in (("scores.csv", "latest.csv"), ("summary.md", "summary.md"), ("worst.png", "worst.png")):
        if os.path.exists(os.path.join(out, f)):
            shutil.copy(os.path.join(out, f), os.path.join(PUB, g))
    shutil.copy(os.path.join(out, "scores.csv"), cache_path(head, bake))
    hp = os.path.join(PUB, "history.csv")
    new = not os.path.exists(hp)
    with open(hp, "a", newline="") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["date", "sha", "pairs", "all_maps", "per_map"])
        w.writerow([datetime.datetime.now().isoformat(timespec="minutes"), head[:10], sum(v["n"] for v in pm.values()),
                    f"{allm:.3f}", " ".join(f"{m}={v['block']:.2f}" for m, v in pm.items())])
    open(os.path.join(PUB, "latest.sha"), "w").write(head + "\n")


def worst_sheet(out, res, n=12):
    """worst.png: the n worst pairs by block error, PS2 | port at half size, a label each."""
    from PIL import Image, ImageDraw
    rows = sorted(res, key=lambda r: -r["block"])[:n]
    cw, ch, lh = W // 2, H // 2, 14
    sh = Image.new("RGB", (2 * cw + 4, len(rows) * (ch + lh)), (0, 0, 0))
    dr = ImageDraw.Draw(sh)
    for i, r in enumerate(rows):
        y = i * (ch + lh)
        dr.text((4, y + 1), f"{r['map']} {r['spot']}  block {r['block']:.1f}  dluma {r['dluma']:+.1f}  dp99 {r['dp99']:+.1f}"
                "   PS2 | port", fill=(255, 255, 255))
        for k, p in enumerate((os.path.join(REF, r["map"], r["spot"] + ".png"), os.path.join(out, "render", r["map"], r["spot"] + ".png"))):
            if os.path.exists(p):
                sh.paste(Image.open(p).convert("RGB").resize((cw, ch), Image.LANCZOS), (k * (cw + 4), y + lh))
    sh.save(os.path.join(out, "worst.png"))


def drop_renders(out):
    """The renders are ~325 MB a full run and re-render byte-identically: delete them once scored."""
    top = os.path.join(out, "render")
    for d, _, files in os.walk(top, topdown=False):
        for f in files:
            if f.endswith(".png"):
                os.remove(os.path.join(d, f))
        if not os.listdir(d):
            os.rmdir(d)


def run_tree(tree, target, out, maps=None, bake=None, keep=False, *, locked=False):
    """Render and score every pose on `tree`'s build; returns (scores, per-map, all-maps mean, head)."""
    if not locked:
        with ci_lock():
            return run_tree(tree, target, out, maps, bake, keep, locked=True)
    tree = os.path.abspath(tree)
    target = os.path.abspath(target or os.path.join(tree, "engine/target"))
    head = git_head(tree)
    rows, skipped = poses(maps)
    if not rows:
        sys.exit(f"no poses for {maps}")
    secs = render(tree, target, out, rows, os.path.abspath(bake or BAKED))
    res = score_all(out, rows)
    expected = {(m, spot) for m, spot, _, _ in rows}
    missing = expected - {(r["map"], r["spot"]) for r in res}
    if missing:
        sys.exit("parity: missing selected scores: " + ", ".join(f"{m}/{spot}" for m, spot in sorted(missing)))
    pm, allm = summary(res, skipped, head, secs, out)
    worst_sheet(out, res)
    if not keep:
        drop_renders(out)
    print(f"parity: {len(res)} pairs, mean block error {allm:.3f}, render {secs:.0f} s -> {out}/scores.csv, summary.md, worst.png",
          flush=True)
    return res, pm, allm, head


def cmd_run(a, *, locked=False):
    if not locked:
        with ci_lock():
            return cmd_run(a, locked=True)
    maps = set(a.maps.split(",")) if a.maps else None
    head = git_head(os.path.abspath(a.tree))
    out = os.path.abspath(a.out or os.path.join(ART, "lead_tmp/parity_runs", f"{datetime.datetime.now():%Y%m%d_%H%M%S}_{head[:8]}"))
    res, pm, allm, head = run_tree(a.tree, a.target, out, maps, a.bake, a.keep_renders, locked=True)
    if a.publish:
        if os.path.abspath(a.tree) != main_root() or maps or a.bake:
            sys.exit("--publish: only a full run of main's checkout over the shared bake is published")
        publish(out, pm, allm, head, bake_fingerprint())
        print(f"published to {PUB}")


def main_tree(root, sha):
    """(tree, target) that builds main's commit `sha`: the main checkout itself while its HEAD is `sha`, else a detached
    worktree beside it (../openbf-wt/parity-main) with its own target, moved to `sha`. Gates run on a branch checked
    out in the main checkout, so its HEAD is not main; scoring that HEAD compared the branch with itself."""
    if git_head(root) == sha:
        return root, None
    wt = os.path.join(os.path.dirname(root), "openbf-wt", "parity-main")
    target = os.path.join(root, "engine/artifacts/lead_tmp/parity_main_target")
    if os.path.isdir(wt):
        r = subprocess.run(["git", "-C", wt, "checkout", "-q", "--detach", sha], capture_output=True, text=True)
    else:
        r = subprocess.run(["git", "-C", root, "worktree", "add", "-q", "--detach", wt, sha], capture_output=True, text=True)
    if r.returncode or git_head(wt) != sha:
        sys.exit(f"parity: cannot put {wt} at main {sha[:10]}:\n{r.stderr[-2000:]}")
    return wt, target


def main_scores(*, locked=False):
    """Path of main's scores.csv (the `main` branch's commit, not the main checkout's HEAD): cached per sha + bake in
    loop/parity/main/; rendered (main_tree) and published when missing. None when main has no pose_render yet. One
    scorer at a time (loop/parity/main.lock)."""
    if not locked:
        with ci_lock():
            return main_scores(locked=True)
    import fcntl
    os.makedirs(os.path.join(PUB, "main"), exist_ok=True)
    with open(os.path.join(PUB, "main.lock"), "w") as lk:
        fcntl.flock(lk, fcntl.LOCK_EX)
        root = main_root()
        head = git_rev(root, "main")
        if not head:
            sys.exit("parity: no `main` branch in the main checkout")
        if subprocess.run(["git", "-C", root, "cat-file", "-e", f"{head}:engine/crates/game/examples/pose_render.rs"],
                          capture_output=True).returncode:
            return None
        bake = bake_fingerprint()
        cache = cache_path(head, bake)
        if os.path.exists(cache):
            return cache
        tree, target = main_tree(root, head)
        out = os.path.join(ART, "lead_tmp/parity_runs", f"main_{head[:10]}")
        res, pm, allm, h2 = run_tree(tree, target, out, locked=True)
        if h2 != head or git_rev(root, "main") != head:
            sys.exit(f"main moved during the parity run ({head[:10]} -> {h2[:10]}) — rerun")
        publish(out, pm, allm, head, bake)
        return cache


def cmd_main(a):
    p = main_scores()
    if not p:
        print("main has no engine/crates/game/examples/pose_render.rs yet — no baseline"); sys.exit(3)
    print(p)


def cmd_gate(a, *, locked=False):
    """Score the tree, score main, compare: exit 0 (no map past the threshold, or only allowed ones), 1 (a map regressed),
    3 (no main baseline). Writes <out>/parity.md and <out>/parity.json (for gates.py)."""
    if not locked:
        with ci_lock():
            return cmd_gate(a, locked=True)
    out = os.path.abspath(a.out)
    allow = set(a.allow.split(",")) if a.allow else set()
    res, pm, allm, head = run_tree(a.tree, a.target, os.path.join(out, "tree"), locked=True)
    base = a.base or main_scores(locked=True)
    if not base:
        json.dump(dict(status="no-baseline", tree=allm), open(os.path.join(out, "parity.json"), "w"))
        open(os.path.join(out, "parity.md"), "w").write(f"tree mean block error {allm:.3f}; main has no pose_render — no baseline\n")
        print("parity gate: no main baseline (main predates pose_render)"); sys.exit(3)
    rows = compare(base, os.path.join(out, "tree", "scores.csv"), None, allow, a.threshold)
    if not rows:
        json.dump(dict(status="error", error="no common pairs"), open(os.path.join(out, "parity.json"), "w"))
        open(os.path.join(out, "parity.md"), "w").write("Parity gate failed: no common pairs\n")
        print("parity gate: no common pairs", file=sys.stderr)
        sys.exit(2)
    baseline = read_csv(base)
    bm = sum(r["block"] for r in baseline) / max(1, len(baseline))
    L = [f"Parity gate: tree {head[:10]} vs main {os.path.basename(base)[:10]} — all-maps mean block error {allm:.3f} "
         f"(main {bm:.3f}); a map FAILS when it rises by more than {a.threshold} unless declared (parity=<maps>).", "",
         "| map | pairs | main | tree | delta | verdict |", "|---|---|---|---|---|---|"]
    L += [f"| {m} | {k} | {mb:.3f} | {mn:.3f} | {d:+.3f} | {v} |" for m, k, mb, mn, d, v in rows]
    open(os.path.join(out, "parity.md"), "w").write("\n".join(L) + "\n")
    json.dump(dict(status="compared", tree=allm, main=bm, base=base,
                   rows=[dict(map=m, pairs=k, main=mb, tree=mn, delta=d, verdict=v) for m, k, mb, mn, d, v in rows]),
              open(os.path.join(out, "parity.json"), "w"), indent=1)
    print("\n".join(L))
    sys.exit(1 if any(r[5] == "FAIL" for r in rows) else 0)


def compare(base, new, maps=None, allow=(), threshold=THRESHOLD):
    """[(map, pairs, base mean, new mean, delta, verdict)], over the (map, spot) pairs both runs scored."""
    b = {(r["map"], r["spot"]): r for r in read_csv(base)}
    n = {(r["map"], r["spot"]): r for r in read_csv(new)}
    keys = sorted(k for k in b.keys() & n.keys() if not maps or k[0] in maps)
    out = []
    for m in sorted({k[0] for k in keys}):
        ks = [k for k in keys if k[0] == m]
        mb = sum(b[k]["block"] for k in ks) / len(ks)
        mn = sum(n[k]["block"] for k in ks) / len(ks)
        d = mn - mb
        v = "PASS" if d <= threshold else ("ALLOWED" if m in allow or "all" in allow else "FAIL")
        out.append((m, len(ks), mb, mn, d, v))
    return out


def cmd_compare(a):
    maps = set(a.maps.split(",")) if a.maps else None
    allow = set(a.allow.split(",")) if a.allow else set()
    rows = compare(a.base, a.new, maps, allow, a.threshold)
    print(f"| map | pairs | base | new | delta | verdict (FAIL above +{a.threshold}) |\n|---|---|---|---|---|---|")
    for m, k, mb, mn, d, v in rows:
        print(f"| {m} | {k} | {mb:.3f} | {mn:.3f} | {d:+.3f} | {v} |")
    if not rows:
        print("no common pairs"); sys.exit(2)
    sys.exit(1 if any(r[5] == "FAIL" for r in rows) else 0)


def scoreboard_line():
    """`parity: mean block error X (was Y), worst 5 posts …` — from loop/parity/history.csv + latest.csv."""
    hp, lp = os.path.join(PUB, "history.csv"), os.path.join(PUB, "latest.csv")
    if not os.path.exists(hp) or not os.path.exists(lp):
        return "parity: no published run yet (`scripts/parity.py main`)"
    h = list(csv.DictReader(open(hp)))
    now = h[-1]
    was = f" (was {float(h[-2]['all_maps']):.2f} at {h[-2]['sha'][:8]})" if len(h) > 1 else ""
    worst = ", ".join(f"{m} {p} {e:.1f}" for e, (m, p) in worst_posts(read_csv(lp)))
    return f"parity: mean block error {float(now['all_maps']):.2f} at {now['sha'][:8]}{was}, {now['pairs']} pairs; worst posts: {worst}"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sp = ap.add_subparsers(dest="cmd", required=True)
    r = sp.add_parser("run")
    r.add_argument("--maps")
    r.add_argument("--tree", default=REPO)
    r.add_argument("--target")
    r.add_argument("--out")
    r.add_argument("--bake", help="a bake root other than engine/assets_baked (a scratch bake)")
    r.add_argument("--publish", action="store_true")
    r.add_argument("--keep-renders", action="store_true")
    sp.add_parser("main")
    g = sp.add_parser("gate")
    g.add_argument("--tree", required=True)
    g.add_argument("--target")
    g.add_argument("--out", required=True)
    g.add_argument("--allow", help="maps allowed to regress (the card's parity=<maps>), or `all`")
    g.add_argument("--threshold", type=float, default=THRESHOLD)
    g.add_argument("--base", help="compare with this scores.csv instead of main HEAD's")
    c = sp.add_parser("compare")
    c.add_argument("base")
    c.add_argument("new")
    c.add_argument("--maps")
    c.add_argument("--allow")
    c.add_argument("--threshold", type=float, default=THRESHOLD)
    sp.add_parser("scoreboard")
    a = ap.parse_args()
    {"run": cmd_run, "main": cmd_main, "gate": cmd_gate, "compare": cmd_compare,
     "scoreboard": lambda _: print(scoreboard_line())}[a.cmd](a)


if __name__ == "__main__":
    main()
