#!/usr/bin/env python3
"""oracle_guard.py — keep builder sessions' hands off the ORACLE: the numbers and files the gates judge a change against.

Why (audits 2026-10-07, OBST / FGHT): a landing that can rewrite its own yardstick (a golden, a PS2 reference, a gates.toml
baseline, the gate code) can make itself pass. Two enforcement points share this file:

  1. A Claude Code PreToolUse hook (Edit|Write|MultiEdit|NotebookEdit|Bash), wired from .claude/settings.json:
         python3 "$CLAUDE_PROJECT_DIR"/scripts/oracle_guard.py hook
     reads the hook JSON on stdin and, in an agent job (OPENBF_AGENT_JOB) that is not exempt, DENIES a write to a protected
     path with a message. It is a tripwire for an honest-but-hasty session, not a sandbox: see docs/agents/README.md.
  2. scripts/gates.py group `protect`: `git diff <base>...HEAD` over the same paths; a build card that changed one without
     declaring it FAILS (declared -> MOVED). This catches what the hook cannot see (a script that writes, a rebase, a commit).

Protected classes (a path is matched by suffix, so a worktree under openbf-wt/ is covered like the main checkout):
  goldens    engine/goldens/**  (except deps.toml, leak_deps.toml, README.md: configuration and prose)
  pcsx2      engine/artifacts/pcsx2/ref/**  and the capture DATA under engine/artifacts/pcsx2 (png mp4 csv json log p2s sav
             state ron); the capture TOOLS (.py .sh) are code and stay editable
  parity     engine/artifacts/loop/parity/**  (the cached per-sha scores parity.py compares against)
  baselines  engine/artifacts/loop/gates.toml  and  engine/artifacts/lead_tmp/loop_baselines/**
  psref      scripts/bot_quality_ps2.json  (the PS2's measured bot numbers)
  guards     scripts/gates.py, scripts/oracle_guard.py, scripts/claude_guard.sh, scripts/land.py, .claude/settings.json

Who may write: OPENBF_ORACLE_EDIT=1 in the harness environment (set by the operator, or by the landing tool around its own
writes — a variable typed inside a Bash command is NOT the harness environment and is itself refused); a session whose job
is `planner` or `land`; a `capture`-phase card for class pcsx2 (capture cards add data); a card whose COMMITTED header says
`declares: oracle=<classes|any>` (read from `git show HEAD:<card>` in the main checkout, so a session cannot declare for
itself by editing its own card file). `goldens=` in `declares:` also frees class goldens (the existing golden declaration).

CLI:  hook                      PreToolUse hook (stdin JSON)
      check <path> ...          which class (if any) each path is in; exit 1 if any is protected
      changed <base> [<tree>]   protected paths changed between <base> and the tree's HEAD, by class
      selftest                  the hook on sample JSON
"""
import json, os, re, shlex, subprocess, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
CARDS = "engine/artifacts/loop/cards/private"
DATA_EXT = r"(?:png|mp4|csv|json|log|p2s|sav|state|ron|gz|zst|bin|dump)"

# (class, regex on a '/'-joined path; matched with re.search so any prefix — worktree or main — is fine)
RULES = [
    ("goldens", r"(?:^|/)engine/goldens/(?!(?:leak_)?deps\.toml$|README\.md$).+"),
    ("pcsx2", r"(?:^|/)engine/artifacts/pcsx2/ref/.+"),
    ("pcsx2", r"(?:^|/)engine/artifacts/pcsx2/(?:[^/]+/)*[^/]+\." + DATA_EXT + r"$"),
    ("parity", r"(?:^|/)engine/artifacts/loop/parity/.+"),
    ("baselines", r"(?:^|/)engine/artifacts/loop/gates\.toml$"),
    ("baselines", r"(?:^|/)engine/artifacts/lead_tmp/loop_baselines/.+"),
    ("psref", r"(?:^|/)scripts/bot_quality_ps2\.json$"),
    ("guards", r"(?:^|/)(?:scripts/(?:gates|oracle_guard|land)\.py|scripts/claude_guard\.sh|\.claude/settings(?:\.local)?\.json)$"),
]
EXEMPT_JOBS = ("planner", "land")
ALL_CLASSES = sorted({c for c, _ in RULES})


def classify(path):
    """The protected class of a path, or None."""
    p = path.replace("\\", "/")
    for cls, rx in RULES:
        if re.search(rx, p) or (not re.search(r"\.[A-Za-z0-9]+/?$", p) and re.search(rx, p.rstrip("/") + "/_")):   # the directory itself counts (rm -r, git checkout --)
            return cls
    return None


def classify_any(path, cwd=None):
    """Check the path as given, made absolute, and with symlinks resolved (a worktree links big dirs to main)."""
    cands = [path]
    if cwd and not os.path.isabs(path):
        cands.append(os.path.join(cwd, path))
    cands = [os.path.normpath(c) for c in cands]
    cands += [os.path.realpath(c) for c in cands]
    for c in cands:
        k = classify(c)
        if k:
            return k
    return None


# ---------- who is exempt / what a card declared ----------
def committed_card(card_id, root=ROOT):
    r = subprocess.run(["git", "-C", root, "show", f"HEAD:{CARDS}/{card_id}.md"], capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else ""


def approved(card_id, key, root=ROOT):
    """A maintainer approval: gates.toml [approvals.<CARD>] <key> = "...".
    Builders cannot edit gates.toml (the hook protects it), so a session cannot approve for itself."""
    try:
        import tomllib
        with open(os.path.join(root, "engine/artifacts/loop/gates.toml"), "rb") as f:
            return str(tomllib.load(f).get("approvals", {}).get(card_id, {}).get(key, "") or "")
    except Exception:
        return ""


def declares_of(text):
    m = re.search(r"^declares:[ \t]*(.*)$", text, re.M)
    return m.group(1).strip() if m else ""


def oracle_declared(declares):
    """Classes a card's `declares:` frees: oracle=<a,b|any>; `goldens=` frees goldens."""
    out = set()
    for part in [p.strip() for p in (declares or "").split(";") if p.strip()]:
        k, _, v = part.partition("=")
        k = k.strip()
        if k == "oracle":
            for x in v.replace(",", " ").split():
                out |= set(ALL_CLASSES) if x == "any" else {x}
        elif k == "goldens" and v.strip():
            out.add("goldens")
    return out


def job_info(env=None):
    env = os.environ if env is None else env
    if env.get("OPENBF_AGENT_JOB"):
        lane = env.get("OPENBF_AGENT_LANE", "build")
        return {"job": "land" if lane == "land" else "card", "card": env.get("OPENBF_CARD")}
    try:
        j = json.loads(env.get("OPENBF_JOB_JSON") or "{}")
    except ValueError:
        j = {}
    return j if isinstance(j, dict) else {}


def allowed_classes(env=None, root=ROOT):
    """(set of classes this session may write, why). 'all' via the env var or an exempt job."""
    env = os.environ if env is None else env
    if env.get("OPENBF_ORACLE_EDIT") == "1":
        return set(ALL_CLASSES), "OPENBF_ORACLE_EDIT=1"
    j = job_info(env)
    if j.get("job") in EXEMPT_JOBS:
        ok = set(ALL_CLASSES)
        if j.get("job") == "land":
            # A landing session gathers the evidence for an EXPOSED claim; it does not approve its own landing (the maintainer, 2026-10-11).
            # Baseline changes require a separate explicit maintainer approval.
            ok.discard("baselines")
        return ok, f"job {j.get('job')}"
    ok = set()
    if j.get("phase") == "capture":
        ok.add("pcsx2")
    cid = j.get("card")
    if cid:
        ap = approved(cid, "oracle", root)   # gates.toml [approvals.<CARD>]: the only channel that works (declarations come from committed cards)
        ok |= oracle_declared(declares_of(committed_card(cid, root)) + (f"; oracle={ap}" if ap else ""))
    return ok, ""


# ---------- the hook ----------
DEST_ARG = {"cp", "mv", "install", "rsync", "ln"}         # a protected SOURCE of cp/rsync is only a read
ANY_ARG = {"tee", "rm", "rmdir", "truncate", "touch", "chmod", "chown", "unlink", "shred", "patch"}
GIT_WRITES = {"checkout", "restore", "rm", "apply", "mv", "stash", "reset", "clean", "am", "cherry-pick", "revert", "merge", "rebase"}


def _segments(cmd):
    return [s for s in re.split(r"\n|;|&&|\|\||\|", cmd) if s.strip()]


def _tokens(seg):
    try:
        return shlex.split(seg, comments=False, posix=True)
    except ValueError:
        return seg.split()


def bash_hits(cmd, cwd=None):
    """[(class, token)] for each protected path a command WRITES (best effort, direct scripts and dynamic paths can escape this best-effort hook)."""
    hits = []
    if "OPENBF_ORACLE_EDIT" in cmd:
        hits.append(("guards", "OPENBF_ORACLE_EDIT (set by the operator, never inside a command)"))
    # redirects: > file, >> file, &> file, >|file, N>file
    for m in re.finditer(r"(?:^|[^<>0-9&\s]|\s)(?:\d?|&)>{1,2}\|?\s*([^\s;&|<>]+)", cmd):
        k = classify_any(m.group(1).strip("'\""), cwd)
        if k:
            hits.append((k, m.group(1)))
    for seg in _segments(cmd):
        t = _tokens(seg)
        while t and (re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", t[0]) or t[0] in ("sudo", "env", "command", "time", "nohup", "flock")):
            t = t[1:]
        if not t:
            continue
        verb = os.path.basename(t[0])
        args = [a for a in t[1:]]
        paths = [a for a in args if not a.startswith("-") or "/" in a]
        if verb in ANY_ARG:
            hits += [(k, a) for a in paths if (k := classify_any(a, cwd))]
        elif verb in DEST_ARG:
            tgt = None
            for i, a in enumerate(args):
                if a in ("-t", "--target-directory") and i + 1 < len(args):
                    tgt = args[i + 1]
            if tgt is None and paths:
                tgt = paths[-1]
            cand = [tgt] if tgt else []
            if verb == "mv":
                cand = paths  # moving a protected path away modifies it too
            hits += [(k, a) for a in cand if (k := classify_any(a, cwd))]
        elif verb in ("sed", "perl", "ruby") and any(re.fullmatch(r"-[A-Za-z]*i[A-Za-z]*\S*", a) or a.startswith("--in-place") for a in args):
            hits += [(k, a) for a in paths if (k := classify_any(a, cwd))]
        elif verb == "dd":
            hits += [(k, a) for a in args if a.startswith("of=") and (k := classify_any(a[3:], cwd))]
        elif verb == "git" and args:
            sub = next((a for a in args if not a.startswith("-")), "")
            if sub in GIT_WRITES:
                hits += [(k, a) for a in args if (k := classify_any(a, cwd))]
    # an interpreter script that names a protected path literal and opens/writes/removes something
    if re.search(r"\b(?:python3?|perl|ruby|node)\b", cmd) and re.search(
            r"open\([^)]*['\"][wa]b?\+?['\"]|write_text|write_bytes|\.write\(|shutil\.|os\.(?:remove|unlink|rename|replace)|\.unlink\(|to_csv|json\.dump|savefig|\.save\(", cmd):
        for lit in re.findall(r"['\"]([^'\"\s;|&<>]*(?:engine/goldens|artifacts/pcsx2|loop/parity|gates\.toml|loop_baselines|bot_quality_ps2|scripts/(?:gates|land|oracle_guard)\.py|claude_guard|\.claude/settings)[^'\"\s;|&<>]*)['\"]", cmd):
            k = classify_any(lit, cwd)
            if k:
                hits.append((k, lit))
    seen, out = set(), []
    for h in hits:
        if h not in seen:
            seen.add(h)
            out.append(h)
    return out


def tool_hits(name, ti, cwd=None):
    if name in ("Edit", "Write", "MultiEdit", "NotebookEdit"):
        p = ti.get("file_path") or ti.get("notebook_path") or ""
        k = classify_any(p, cwd) if p else None
        return [(k, p)] if k else []
    if name == "Bash":
        return bash_hits(ti.get("command", ""), cwd)
    return []


def decide(d, env=None, root=ROOT):
    """(deny reason or None) for one hook payload."""
    env = os.environ if env is None else env
    if not (env.get("OPENBF_AGENT_JOB") or env.get("OPENBF_LOOP") == "1") or env.get("OPENBF_ORACLE_GUARD") == "0":
        return None
    hits = tool_hits(d.get("tool_name", ""), d.get("tool_input", {}) or {}, d.get("cwd"))
    if not hits:
        return None
    ok, _why = allowed_classes(env, root)
    bad = [(k, p) for k, p in hits if k not in ok]
    if not bad:
        return None
    lines = "; ".join(f"{p} [{k}]" for k, p in bad[:4])
    return (f"openbf oracle guard: this would change the yardstick the gates judge against ({lines}). A builder session does "
            f"not rewrite goldens, PS2 references, parity scores, gates.toml baselines or the gate/hook code. If this card really "
            f"must, block it with `needs: maintainer` and ask for `[approvals.<CARD>] oracle = \"{','.join(sorted({k for k, _ in bad}))}\"` in "
            f"engine/artifacts/loop/gates.toml (the planner or the maintainer adds it; a card header is not read); re-baselines go through scripts/land.py. Not for you to work around "
            f"(scripts/AUTOLOOP.md, section Oracle protection).")


def hook():
    try:
        d = json.load(sys.stdin)
    except ValueError:
        return 0
    why = decide(d)
    if why:
        print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny",
                                                  "permissionDecisionReason": why}}))
    return 0


# ---------- gates.py's half ----------
class ComparisonError(RuntimeError):
    """The protected-path comparison was unavailable, rather than empty."""


def changed_by_class(tree, base):
    """{class: [paths]} the tree's HEAD changed since <base> (three-dot: its own commits)."""
    cmd = ["git", "-C", tree, "diff", "--name-only", f"{base}...HEAD"]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True)
    except OSError as e:
        raise ComparisonError(f"{' '.join(cmd)} unavailable: {e}") from e
    if r.returncode:
        raise ComparisonError(f"git -C {tree} diff --name-only {base}...HEAD failed ({r.returncode}): {r.stderr.strip()}")
    out = {}
    for f in r.stdout.split():
        k = classify(f)
        if k:
            out.setdefault(k, []).append(f)
    return out


def card_declares_for(env=None, tree=None, root=ROOT):
    """The declared-oracle classes of the card this gates run belongs to: OPENBF_CARD (or a legacy OPENBF_JOB_JSON card), else the card whose
    `branch:` is the tree's branch. Read from the COMMITTED card in the main checkout."""
    env = os.environ if env is None else env
    cid = job_info(env).get("card")
    if not cid and tree:
        br = subprocess.run(["git", "-C", tree, "rev-parse", "--abbrev-ref", "HEAD"], capture_output=True, text=True).stdout.strip()
        cdir = os.path.join(root, CARDS)
        for f in sorted(os.listdir(cdir)) if os.path.isdir(cdir) else []:
            if not f.endswith(".md"):
                continue
            txt = open(os.path.join(cdir, f), errors="replace").read(4000)
            m = re.search(r"^branch:[ \t]*(\S+)", txt, re.M)
            if m and m.group(1) == br:
                cid = f[:-3]
                break
    if not cid:
        return None, set()
    d, ap = declares_of(committed_card(cid, root)), approved(cid, "oracle", root)
    return cid, oracle_declared(d + (f"; oracle={ap}" if ap else ""))


# ---------- self test ----------
def selftest():
    bad = 0

    def case(name, payload, env, want_deny):
        nonlocal bad
        why = decide(payload, env, ROOT)
        ok = bool(why) == want_deny
        bad += not ok
        print(f"{'ok  ' if ok else 'FAIL'} {name}: {'DENY' if why else 'allow'}" + ("" if ok else f"  (wanted {'deny' if want_deny else 'allow'}) {why}"))

    agent = {"OPENBF_AGENT_JOB": "guard-fixture", "OPENBF_CARD": "ZZZNOCARD", "OPENBF_AGENT_LANE": "build"}
    case("agent build protects baseline", {"tool_name": "Write", "tool_input": {"file_path": ROOT + "/engine/artifacts/loop/gates.toml"}}, agent, True)
    case("agent land protects baseline", {"tool_name": "Write", "tool_input": {"file_path": ROOT + "/engine/artifacts/loop/gates.toml"}}, dict(agent, OPENBF_AGENT_LANE="land"), True)
    case("agent ordinary source allowed", {"tool_name": "Write", "tool_input": {"file_path": ROOT + "/scripts/example.py"}}, agent, False)
    loop = {"OPENBF_LOOP": "1", "OPENBF_JOB_JSON": json.dumps({"job": "card", "card": "ZZZNOCARD", "phase": "build"})}
    wt = os.path.join(os.path.dirname(ROOT), "openbf-wt", "obsr") + "/"
    E = lambda p: {"tool_name": "Edit", "tool_input": {"file_path": p, "old_string": "a", "new_string": "b"}}
    W = lambda p: {"tool_name": "Write", "tool_input": {"file_path": p, "content": "x"}}
    B = lambda c: {"tool_name": "Bash", "tool_input": {"command": c}}
    case("Edit golden in worktree", E(wt + "engine/goldens/maps/bf1_kas2.png"), loop, True)
    case("Write gates.toml baseline", W(ROOT + "/engine/artifacts/loop/gates.toml"), loop, True)
    case("Write parity cache", W(ROOT + "/engine/artifacts/loop/parity/main/abc.csv"), loop, True)
    case("Edit PS2 ref json", E(ROOT + "/engine/artifacts/pcsx2/ref/bf1_kas2/x.json"), loop, True)
    case("Edit gates.py", E(wt + "scripts/gates.py"), loop, True)
    case("Edit bot_quality_ps2.json", E(wt + "scripts/bot_quality_ps2.json"), loop, True)
    case("Edit game source", E(wt + "engine/crates/core/src/ai.rs"), loop, False)
    case("Edit deps.toml (config)", E(wt + "engine/goldens/deps.toml"), loop, False)
    case("Edit pcsx2 tool .py", E(ROOT + "/engine/artifacts/pcsx2/pose.py"), loop, False)
    case("Bash redirect into golden", B("echo x > engine/goldens/maps/a.png"), loop, True)
    case("Bash cp onto golden", B("cp /tmp/new.png engine/goldens/maps/a.png"), loop, True)
    case("Bash cp FROM golden (read)", B("cp engine/goldens/maps/a.png /tmp/a.png"), loop, False)
    case("Bash mv golden", B("mv engine/goldens/maps/a.png /tmp/"), loop, True)
    case("Bash rm baseline", B("rm engine/artifacts/lead_tmp/loop_baselines/20261007_1022_OBSR/sweep_main_1.txt"), loop, True)
    case("Bash sed -i gates.toml", B("sed -i 's/passed  = 1/passed = 2/' engine/artifacts/loop/gates.toml"), loop, True)
    case("Bash git checkout -- goldens", B("git checkout -- engine/goldens"), loop, True)
    case("Bash python writes golden", B("python3 - <<'E'\nopen('engine/goldens/walkabout_stats.ron','w').write('x')\nE"), loop, True)
    case("Bash cat gates.toml (read)", B("cat engine/artifacts/loop/gates.toml | head"), loop, False)
    fp = ("python3 - <<'EOF'\ns=open('parked.py').read()\nopen('parked.py','w').write(s)\nEOF\ngrep -n x parked.py | head -2; timeout 400 python3 parked.py "
          + ROOT + "/engine/artifacts/pcsx2/ref/states/a.p2s 1 2 3 > /tmp/x.log 2>&1; echo EXIT=$?; grep -E \"^yaw\" /tmp/x.log")
    case("Bash edits a scratch script AND reads a PS2 state in one command (was a false deny)", B(fp), loop, False)
    case("Bash python writes a pcsx2 ref file by literal path", B("python3 - <<'E'\nopen('engine/artifacts/pcsx2/ref/x.json','w').write('x')\nE"), loop, True)
    case("Bash gates.py run (mentions nothing protected)", B("python3 scripts/gates.py --out /tmp/x --auto"), loop, False)
    case("Bash env-var bypass typed in command", B("OPENBF_ORACLE_EDIT=1 cp a engine/goldens/x.png"), loop, True)
    case("Bash cargo test", B("cd engine && cargo test -p core 2>&1 | tail"), loop, False)
    case("not a loop session", E(wt + "engine/goldens/maps/a.png"), {}, False)
    case("OPENBF_ORACLE_EDIT=1 in env", E(wt + "engine/goldens/maps/a.png"), dict(loop, OPENBF_ORACLE_EDIT="1"), False)
    case("planner job", E(ROOT + "/engine/artifacts/loop/gates.toml"),
         {"OPENBF_LOOP": "1", "OPENBF_JOB_JSON": json.dumps({"job": "planner"})}, False)
    case("land job cannot write gates.toml itself (no self-approval)", E(ROOT + "/engine/artifacts/loop/gates.toml"),
         {"OPENBF_LOOP": "1", "OPENBF_JOB_JSON": json.dumps({"job": "land"})}, True)
    case("land job still commits a golden", E(ROOT + "/engine/goldens/maps/a.png"),
         {"OPENBF_LOOP": "1", "OPENBF_JOB_JSON": json.dumps({"job": "land"})}, False)
    case("land job runs land.py (the script re-baselines)", B("python3 scripts/land.py ZZZ --commit"),
         {"OPENBF_LOOP": "1", "OPENBF_JOB_JSON": json.dumps({"job": "land"})}, False)
    cap = {"OPENBF_LOOP": "1", "OPENBF_JOB_JSON": json.dumps({"job": "card", "card": "ZZZNOCARD", "phase": "capture"})}
    case("capture-phase card adds pcsx2 data", W(ROOT + "/engine/artifacts/pcsx2/ref/bf1_kas2/new.json"), cap, False)
    case("capture-phase card still not goldens", W(ROOT + "/engine/goldens/maps/a.png"), cap, True)
    # a declared card (fixture repo with a committed card)
    import tempfile
    with tempfile.TemporaryDirectory() as t:
        os.makedirs(os.path.join(t, CARDS))
        open(os.path.join(t, CARDS, "DCL.md"), "w").write("---\nid: DCL\ndeclares: sweep; oracle=guards\n---\n")
        subprocess.run(["git", "init", "-q", t])
        subprocess.run(["git", "-C", t, "add", "-A"])
        subprocess.run(["git", "-C", t, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "c"])
        env = {"OPENBF_LOOP": "1", "OPENBF_JOB_JSON": json.dumps({"job": "card", "card": "DCL", "phase": "build"})}
        for name, p, want in (("declared oracle=guards frees gates.py", wt + "scripts/gates.py", False),
                              ("...but not goldens", wt + "engine/goldens/maps/a.png", True)):
            why = decide(E(p), env, t)
            ok = bool(why) == want
            bad += not ok
            print(f"{'ok  ' if ok else 'FAIL'} {name}: {'DENY' if why else 'allow'}")
    import tempfile   # approvals live in gates.toml [approvals.<CARD>] (declarations come from committed cards, so a header cannot carry them)
    t = tempfile.mkdtemp()
    os.makedirs(os.path.join(t, "engine/artifacts/loop"))
    open(os.path.join(t, "engine/artifacts/loop/gates.toml"), "w").write('[approvals.AAA]\nfidelity = "recycled, x@m"\noracle = "baselines"\n')
    ok = (approved("AAA", "fidelity", t) == "recycled, x@m" and approved("BBB", "fidelity", t) == ""
          and card_declares_for({"OPENBF_JOB_JSON": json.dumps({"card": "AAA"})}, None, t) == ("AAA", {"baselines"})
          and card_declares_for({"OPENBF_JOB_JSON": json.dumps({"card": "BBB"})}, None, t) == ("BBB", set()))
    bad += not ok
    print(f"{'ok  ' if ok else 'FAIL'} approvals: read from gates.toml [approvals.<CARD>], per card")
    E2 = lambda p: {"tool_name": "Edit", "tool_input": {"file_path": p, "old_string": "a", "new_string": "b"}}
    envc = lambda c: {"OPENBF_LOOP": "1", "OPENBF_JOB_JSON": json.dumps({"job": "card", "card": c, "phase": "build"})}
    open(os.path.join(t, "engine/artifacts/loop/gates.toml"), "w").write('[approvals.AAA]\noracle = "baselines,guards"\n')
    ok = (not decide(E2(os.path.join(t, "scripts/gates.py")), envc("AAA"), t) and not decide(E2(os.path.join(t, "engine/artifacts/loop/gates.toml")), envc("AAA"), t)
          and bool(decide(E2(os.path.join(t, "engine/goldens/x.png")), envc("AAA"), t)) and bool(decide(E2(os.path.join(t, "scripts/gates.py")), envc("BBB"), t)))
    bad += not ok
    print(f"{'ok  ' if ok else 'FAIL'} the HOOK honours an approval: the approved card edits gates.py / gates.toml, not goldens; another card is denied")
    print("selftest:", "FAILED" if bad else "all passed")
    return 1 if bad else 0


def main():
    a = sys.argv[1:]
    if not a or a[0] == "hook":
        return hook()
    if a[0] == "check":
        rc = 0
        for p in a[1:]:
            k = classify_any(p)
            print(f"{k or '-':10s} {p}")
            rc |= bool(k)
        return rc
    if a[0] == "changed":
        try:
            r = changed_by_class(a[2] if len(a) > 2 else ROOT, a[1])
        except ComparisonError as e:
            print(str(e), file=sys.stderr)
            return 1
        print(json.dumps(r, indent=1))
        return 1 if r else 0
    if a[0] == "selftest":
        return selftest()
    sys.exit(__doc__)


if __name__ == "__main__":
    sys.exit(main())
