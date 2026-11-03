#!/usr/bin/env python3
"""Read and maintain historical audit prerequisite holds.

The launcher, scheduler and claims are retired. Use cards.py for issue cards and
agent_job.sh for jobs. This module remains for acceptance records in
the acceptance index that call audit_holds or the commands below.

  loopctl.py audit-status
  loopctl.py audit-checkpoint TARGET --missing PATH --revision REV --reason TEXT --report PATH
  loopctl.py audit-reset TARGET --reviewed-by NAME --reason TEXT

Restored inputs and a reviewed reset never constitute an audit verdict.
"""
import argparse
from contextlib import contextmanager
import datetime
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
LOOP = Path(os.environ.get("OPENBF_LOOP_DIR") or ROOT / "engine/artifacts/loop")
AUDIT_HOLDS = LOOP / "audit_holds.json"
JOURNAL = LOOP / "JOURNAL.md"


def main_checkout():
    common = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        check=True, capture_output=True, text=True).stdout.strip()
    return Path(common).parent


AUDIT_ROOT = Path(os.environ.get("OPENBF_AUDIT_ROOT") or main_checkout())


@contextmanager
def audit_lock():
    """Serialize audit evidence edits without the retired launcher claim system."""
    LOOP.mkdir(parents=True, exist_ok=True)
    with (LOOP / "audit-holds.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


def audit_holds():
    """Separate incomplete checkpoints from earned audit verdicts."""
    try:
        return json.loads(Path(AUDIT_HOLDS).read_text())
    except FileNotFoundError:
        return {}


def write_audit_holds(holds):
    # Caller holds audit_lock; readers never observe a partial JSON document.
    path = Path(AUDIT_HOLDS)
    pending = path.with_suffix(".tmp")
    pending.write_text(json.dumps(holds, indent=2, sort_keys=True) + "\n")
    pending.replace(path)


def audit_checkpoint(target, missing, revision, reason, report):
    """Record input existence only; retain every earlier prerequisite."""
    if not all(x.strip() for x in (target, revision, reason, report)) or not missing:
        raise ValueError("target, missing prerequisites, revision, reason and report are required")
    if not (Path(AUDIT_ROOT) / report).is_file():
        raise ValueError(f"checkpoint report does not exist: {report}")
    paths = list(dict.fromkeys(missing))
    if any(not p.strip() or (Path(AUDIT_ROOT) / p).exists() for p in paths):
        raise ValueError("--missing must name exact paths that are currently absent")
    with audit_lock():
        holds = audit_holds()
        paths = list(dict.fromkeys(holds.get(target, {}).get("missing", []) + paths))
        holds[target] = dict(target=target, missing=paths, revision=revision, reason=reason, report=report,
                             recorded=datetime.datetime.now(datetime.timezone.utc).isoformat())
        write_audit_holds(holds)


def audit_reset(target, reviewed_by, reason):
    if not reviewed_by.strip() or not reason.strip():
        raise ValueError("reviewed reset requires --reviewed-by and --reason")
    with audit_lock():
        holds = audit_holds()
        if target not in holds:
            raise ValueError(f"no audit hold for {target}")
        del holds[target]
        write_audit_holds(holds)
        with Path(JOURNAL).open("a") as journal:
            journal.write(f"- AUDIT RESET {target}: reviewed by {reviewed_by} — {reason}\n")


def audit_hold_reason(target, holds):
    hold = holds.get(target)
    if not hold:
        return ""
    missing = [p for p in hold["missing"] if not (Path(AUDIT_ROOT) / p).exists()]
    if not missing:
        return ""
    return (f"INCOMPLETE / UNAUDITED: {hold['reason']}; restore ALL: {', '.join(missing)} "
            f"(checkpoint {hold['revision']}, report {hold['report']}); or reviewed reset: "
            f"loopctl.py audit-reset {target} --reviewed-by NAME --reason TEXT")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("audit-status")
    checkpoint = commands.add_parser("audit-checkpoint")
    checkpoint.add_argument("target")
    checkpoint.add_argument("--missing", action="append", required=True)
    for field in ("revision", "reason", "report"):
        checkpoint.add_argument(f"--{field}", required=True)
    reset = commands.add_parser("audit-reset")
    reset.add_argument("target")
    reset.add_argument("--reviewed-by", required=True)
    reset.add_argument("--reason", required=True)
    args = parser.parse_args()
    try:
        if args.command == "audit-status":
            holds = audit_holds()
            print(f"{len(holds)} historical prerequisite holds; path existence is not an audit verdict.")
            for target in sorted(holds):
                print(f"{target}: " + (audit_hold_reason(target, holds) or "inputs restored; remains UNAUDITED"))
        elif args.command == "audit-checkpoint":
            audit_checkpoint(args.target, args.missing, args.revision, args.reason, args.report)
            print(f"{args.target}: incomplete checkpoint recorded; remains UNAUDITED")
        else:
            audit_reset(args.target, args.reviewed_by, args.reason)
            print(f"{args.target}: reviewed prerequisite hold cleared; remains UNAUDITED")
    except (ValueError, OSError) as exc:
        print(f"audit holds: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
