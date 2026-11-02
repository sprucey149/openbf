#!/usr/bin/env python3
"""A new provisional value must say what measurement replaces it.

A value labelled PROVISIONAL, OURS or INTERIM reads as finished once nobody remembers it was a guess.
So a line ADDED with one of those labels must carry `replace:`, `measure:` or `trace:` in the same
comment, naming the capture, disc read or run that will settle it:

    // PROVISIONAL, replace: a PS2 trace of the hero's yaw rate
    const TURN_RATE: f32 = 12.0;

Lines that were already there are not checked, so this stays quiet about old debt and speaks only to
new work.

    scripts/check_provisional.py               # the working tree against HEAD
    scripts/check_provisional.py main...HEAD   # what a branch would bring in (three dots: not what main gained since)
    scripts/check_provisional.py --selftest
"""
import re
import subprocess
import sys

LABEL = re.compile(r"\b(PROVISIONAL|OURS|INTERIM)\b")
PLAN = re.compile(r"(replace:|measure:|trace:)", re.I)
COMMENT = re.compile(r"^\s*(//|#|\*)")
PATHS = ["engine/crates", "scripts"]


def added_blocks(diff):
    """Yield (path, first line number, lines) for each hunk's run of added lines."""
    path, line, block, first = None, 0, [], 0
    for raw in diff.split("\n"):
        if raw.startswith("+++ "):
            if block:
                yield path, first, block
                block = []
            path = raw[6:]
        elif raw.startswith("@@"):
            if block:
                yield path, first, block
                block = []
            line = int(re.match(r"@@ -\d+(?:,\d+)? \+(\d+)", raw).group(1))
        elif raw.startswith("+"):
            if not block:
                first = line
            block.append(raw[1:])
            line += 1
    if block:
        yield path, first, block


def findings(diff):
    """Added labels with no plan in their comment: the labelled line and the comment lines touching it."""
    bad = []
    for path, first, lines in added_blocks(diff):
        if path is None or path.endswith("check_provisional.py"):
            continue
        for i, text in enumerate(lines):
            if not LABEL.search(text):
                continue
            low = i
            while low > 0 and COMMENT.match(lines[low - 1]):
                low -= 1
            high = i
            while high + 1 < len(lines) and COMMENT.match(lines[high + 1]):
                high += 1
            if not PLAN.search("\n".join(lines[low : high + 1])):
                bad.append(f"{path}:{first + i}: {text.strip()[:140]}")
    return bad


def selftest():
    def one(text):
        return f"+++ b/engine/crates/x.rs\n@@ -0,0 +1,{len(text)} @@\n" + "".join(f"+{t}\n" for t in text)

    assert findings(one(["const A: f32 = 1.0; // PROVISIONAL"]))
    assert not findings(one(["const A: f32 = 1.0; // PROVISIONAL, measure: a PS2 trace of A"]))
    assert not findings(one(["// INTERIM until the disc is read.", "// replace: the disc read of A", "const A: u8 = 1;"]))
    assert findings(one(["// OURS.", "const A: u8 = 1;", "// replace: unrelated, not this comment"]))
    assert not findings(one(["let hours = 1;"])), "a label is a whole word"
    two = "+++ b/a.rs\n@@ -0,0 +1 @@\n+// OURS\n+++ b/b.rs\n@@ -0,0 +1 @@\n+let b = 1;\n"
    assert findings(two) == ["a.rs:1: // OURS"], "a file's last block is that file's"
    skip = "+++ b/check_provisional.py\n@@ -0,0 +1 @@\n+// OURS\n+++ b/b.rs\n@@ -0,0 +1 @@\n+let b = 1;\n"
    assert not findings(skip), "the checker's own lines stay its own"
    print("check_provisional: selftest ok")


def main():
    args = sys.argv[1:]
    if args == ["--selftest"]:
        selftest()
        return 0
    diff = subprocess.run(
        ["git", "diff", "-U0", *(args or ["HEAD"]), "--", *PATHS],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    bad = findings(diff)
    for b in bad:
        print(b)
    print(f"check_provisional: {len(bad)} new provisional value(s) without a replace:/measure:/trace: plan")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
