#!/usr/bin/env python3
"""Run the detailed bench over every baked map and table what it measured.

Per map: one sim-only run (load, setup, ticks, memory), then one run per adapter and camera (frames).
All of it is one process at a time, pinned to the performance cores so the instruction counter counts.

    scripts/perf_matrix.py run --bin <game> --out <dir> [--assets engine/assets_baked] [--maps a,b]
    scripts/perf_matrix.py table <dir>            # worst figures per map, and the worst map per column
    scripts/perf_matrix.py compare <before> <after>  # same, before -> after, and whether the sim's hashes moved

Instructions are the firm numbers: another job moves milliseconds, not instruction counts. Build
--features alloc-count to fill the allocation columns; that build's instruction counts include the counter.
"""
import argparse
import json
import os
import pathlib
import subprocess
import sys

# Everything in a detail report that says what the sim did, as opposed to how long it took.
SIM_FIELDS = ["initial_hash", "sim_hash", "tick_hashes", "camera_samples", "nav_floors", "nav_unwalkable",
              "nav_trapped", "kill_floor", "alive_after_sim", "initial_alive", "final_hash"]


def maps(assets, only):
    if only:
        return only.split(",")
    return sorted(d.name for d in pathlib.Path(assets).iterdir() if (d / "manifest.ron").exists() or (d / "manifest.json").exists())


def bench(args, m, name, extra, env=None):
    out = pathlib.Path(args.out) / f"{m}.{name}.json"
    if out.exists() and not args.again:
        return
    cmd = ["taskset", "-c", args.cores, args.bin, "--map", str(pathlib.Path(args.assets) / m), "--bench",
           "--seed", str(args.seed), "--ai", str(args.per_side), "--ai-cap", str(args.per_side * 2)] + extra
    base = dict(os.environ, OPENBF_BENCH_DETAIL=str(out), **(env or {}))
    # A map may not seat the full count at the opening: then it runs with what it opens with.
    for extra_env in ({"OPENBF_BENCH_TOTAL_PER_SIDE": "1"}, {}):
        p = subprocess.run(cmd, env=dict(base, **extra_env), capture_output=True, text=True, timeout=args.timeout)
        if p.returncode == 0:
            print(m, name, "ok" if extra_env else "ok (opening population below the request)", flush=True)
            return
    print(m, name, "FAILED", p.stderr.strip()[-300:], flush=True)


def run(args):
    pathlib.Path(args.out).mkdir(parents=True, exist_ok=True)
    for m in maps(args.assets, args.maps):
        bench(args, m, "sim", ["--bench-ticks", str(args.ticks), "--bench-frames", "0"])
        for adapter in args.adapters.split(","):
            for cam, flag in (("eye", ["--bench-player-cam"]), ("overview", [])):
                if cam in args.cams.split(","):
                    bench(args, m, f"{adapter}.{cam}", ["--bench-ticks", str(args.warm_ticks), "--bench-frames",
                          str(args.frames), "--size", args.size] + flag,
                          {"OPENBF_ADAPTER": adapter, "OPENBF_GPU_TIMING": "1"})


def pct(values, q):
    v = sorted(x for x in values if x is not None)
    if not v:
        return None
    return v[min(len(v) - 1, max(0, int(q * len(v) + 0.999999) - 1))]


def mean(values):
    v = [x for x in values if x is not None]
    return sum(v) / len(v) if v else None


def stats(values):
    return {"mean": mean(values), "p99": pct(values, 0.99), "max": pct(values, 1.0)}


def load(path):
    return json.loads(pathlib.Path(path).read_text())


def summarize(dir_):
    """{map: {run name: figures}} for every report in a directory."""
    out = {}
    for path in sorted(pathlib.Path(dir_).glob("*.json")):
        m, name = path.name[:-5].split(".", 1)
        r = load(path)
        s = {"load_ms": r.get("load_ms"), "setup_ms": r.get("setup_ms"), "upload_ms": r.get("upload_ms"),
             "peak_mb": (r.get("memory_end_kb", {}).get("peak") or 0) / 1024, "alive": r.get("initial_alive"),
             "gpu_mb": sum(o.get("bytes", 0) for o in r.get("load_objects", [])) / 1048576}
        ticks = r.get("ticks", [])
        for k in ("cpu_ms", "instr", "allocs", "alloc_bytes"):
            s["tick_" + k] = stats([t.get(k) for t in ticks])
        frames = r.get("frames", [])[2:]  # the first frames warm caches the loading screen does not cover
        if frames:
            s["submit_ms"] = stats([f.get("submit", {}).get("cpu_ms") for f in frames])
            s["submit_instr"] = stats([f.get("submit", {}).get("instr") for f in frames])
            s["submit_allocs"] = stats([f.get("submit", {}).get("allocs") for f in frames])
            s["update_ms"] = stats([f.get("update", {}).get("cpu_ms") for f in frames])
            s["update_instr"] = stats([f.get("update", {}).get("instr") for f in frames])
            s["update_allocs"] = stats([f.get("update", {}).get("allocs") for f in frames])
            s["gpu_ms"] = stats([sum(x[1] for x in f.get("gpu_ms", [])) or None for f in frames])
            s["wall_ms"] = stats([f.get("wall_ms") for f in frames])
            s["draws"] = mean([f.get("draw_calls") for f in frames])
        out.setdefault(m, {})[name] = s
    return out


def f(x, scale=1.0, digits=2):
    return "-" if x is None else f"{x / scale:.{digits}f}"


def sim_row(m, s):
    t, i, a = s["tick_cpu_ms"], s["tick_instr"], s["tick_allocs"]
    return (f"| {m} | {s['alive']} | {f(s['load_ms'],1,0)} | {f(s['setup_ms'],1,0)} | {f(t['mean'])} | {f(t['p99'])} | "
            f"{f(t['max'])} | {f(i['mean'],1e6,1)} | {f(i['max'],1e6,1)} | {f(a['mean'],1,1)} | {f(a['max'],1,0)} | {s['peak_mb']:.0f} |")


def frame_row(m, name, s):
    c, i, g, w, a = s["submit_ms"], s["submit_instr"], s["gpu_ms"], s["wall_ms"], s["submit_allocs"]
    return (f"| {m} | {name} | {f(s['draws'],1,0)} | {f(c['mean'])} | {f(c['max'])} | {f(i['mean'],1e6,1)} | {f(i['max'],1e6,1)} | "
            f"{f(a['mean'],1,0)} | {f(g['mean'])} | {f(g['p99'])} | {f(g['max'])} | {f(w['p99'])} | {f(w['max'])} | "
            f"{f(s['upload_ms'],1,0)} | {s['gpu_mb']:.0f} | {s['peak_mb']:.0f} |")


def table(args):
    data = summarize(args.dir)
    print("| Map | Alive | Load ms | Setup ms | Tick CPU mean | p99 | max | M instr mean | max | Allocs/tick mean | max | Peak MB |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|")
    for m in sorted(data):
        if "sim" in data[m]:
            print(sim_row(m, data[m]["sim"]))
    print()
    print("| Map | Run | Draws | Submit CPU mean | max | M instr mean | max | Allocs/frame | GPU mean | p99 | max | Wall p99 | max | Upload ms | GPU MB made | Peak MB |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for m in sorted(data):
        for name, s in sorted(data[m].items()):
            if name != "sim" and "submit_ms" in s:
                print(frame_row(m, name, s))


def change(a, b):
    if a is None or b is None or a == 0:
        return "-"
    return f"{(b - a) / a * 100:+.1f}%"


def compare(args):
    before, after = summarize(args.before), summarize(args.after)
    moved = 0
    checked = 0
    for path in sorted(pathlib.Path(args.before).glob("*.json")):
        other = pathlib.Path(args.after) / path.name
        if not other.exists():
            continue
        a, b = load(path), load(other)
        checked += 1
        for k in SIM_FIELDS:
            if a.get(k) != b.get(k):
                moved += 1
                print(f"SIM MOVED {path.name}: {k}")
    print(f"sim fields compared on {checked} runs: {moved} differ\n")
    print("| Map | Tick instr mean | Tick instr max | Tick CPU max | Allocs/tick mean | Load+setup ms | Peak MB |")
    print("|---|---|---|---|---|---|---|")
    for m in sorted(before):
        if m in after and "sim" in before[m] and "sim" in after[m]:
            a, b = before[m]["sim"], after[m]["sim"]
            print(f"| {m} | {f(a['tick_instr']['mean'],1e6,1)} -> {f(b['tick_instr']['mean'],1e6,1)} ({change(a['tick_instr']['mean'], b['tick_instr']['mean'])}) | "
                  f"{f(a['tick_instr']['max'],1e6,1)} -> {f(b['tick_instr']['max'],1e6,1)} | {f(a['tick_cpu_ms']['max'])} -> {f(b['tick_cpu_ms']['max'])} | "
                  f"{f(a['tick_allocs']['mean'],1,1)} -> {f(b['tick_allocs']['mean'],1,1)} | "
                  f"{f((a['load_ms'] or 0)+(a['setup_ms'] or 0),1,0)} -> {f((b['load_ms'] or 0)+(b['setup_ms'] or 0),1,0)} | {a['peak_mb']:.0f} -> {b['peak_mb']:.0f} |")
    print()
    print("| Map | Run | Submit M instr mean | max | Update M instr mean | Submit CPU p99 | GPU mean | GPU max | Wall max |")
    print("|---|---|---|---|---|---|---|---|---|")
    for m in sorted(before):
        for name in sorted(before[m]):
            a, b = before[m][name], after.get(m, {}).get(name)
            if name == "sim" or not b or "submit_ms" not in a or "submit_ms" not in b:
                continue
            print(f"| {m} | {name} | {f(a['submit_instr']['mean'],1e6,1)} -> {f(b['submit_instr']['mean'],1e6,1)} ({change(a['submit_instr']['mean'], b['submit_instr']['mean'])}) | "
                  f"{f(a['submit_instr']['max'],1e6,1)} -> {f(b['submit_instr']['max'],1e6,1)} | "
                  f"{f(a['update_instr']['mean'],1e6,1)} -> {f(b['update_instr']['mean'],1e6,1)} ({change(a['update_instr']['mean'], b['update_instr']['mean'])}) | "
                  f"{f(a['submit_ms']['p99'])} -> {f(b['submit_ms']['p99'])} | {f(a['gpu_ms']['mean'])} -> {f(b['gpu_ms']['mean'])} | "
                  f"{f(a['gpu_ms']['max'])} -> {f(b['gpu_ms']['max'])} | {f(a['wall_ms']['max'])} -> {f(b['wall_ms']['max'])} |")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--bin", required=True)
    r.add_argument("--out", required=True)
    r.add_argument("--assets", default="engine/assets_baked")
    r.add_argument("--maps", default="")
    r.add_argument("--adapters", default="igpu,dgpu")
    r.add_argument("--cams", default="eye,overview")
    r.add_argument("--per-side", type=int, default=32)
    r.add_argument("--seed", type=int, default=2)
    r.add_argument("--ticks", type=int, default=1800)
    r.add_argument("--warm-ticks", type=int, default=300)
    r.add_argument("--frames", type=int, default=240)
    r.add_argument("--size", default="1280x720")
    r.add_argument("--cores", default="0-15")
    r.add_argument("--timeout", type=int, default=900)
    r.add_argument("--again", action="store_true", help="redo runs whose report already exists")
    t = sub.add_parser("table")
    t.add_argument("dir")
    c = sub.add_parser("compare")
    c.add_argument("before")
    c.add_argument("after")
    args = p.parse_args()
    {"run": run, "table": table, "compare": compare}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
