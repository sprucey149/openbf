"""Issue-card contracts and retained historical prerequisite evidence."""
import contextlib
from concurrent.futures import ThreadPoolExecutor
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cards
import loopctl


class Cards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.card_dir = self.root / "cards/private"
        self.card_dir.mkdir(parents=True)
        for name, value in (("ROOT", self.root), ("LOOP", self.root), ("CARDS", self.card_dir)):
            patcher = patch.object(cards, name, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def record(self, cid="ISSUE-7", issue="7", **extra):
        return dict(id=cid, issue=issue, title="Technical scope", status="todo", _private=True,
                    _path=str(self.card_dir / f"{cid}.md"), _body="\nEvidence remains unchecked.\n", **extra)

    def test_private_roundtrip_and_backlog(self):
        record = self.record()
        cards.write_card(record)
        loaded = cards.cards()
        self.assertEqual(cards.check(loaded), [])
        self.assertEqual(loaded["ISSUE-7"]["_body"].strip(), record["_body"].strip())
        cards.write_backlog(loaded)
        backlog = (self.root / "BACKLOG.md").read_text()
        self.assertTrue(backlog.startswith(cards.PRIVATE_START))
        self.assertIn("issues/7", backlog)
        self.assertIn("cards/private/ISSUE-7.md", backlog)
        self.assertTrue(backlog.endswith(cards.PRIVATE_END))

    def test_path_validation_distinguishes_new_paths(self):
        record = self.record(files="missing.py, planned.py (new)")
        errors = cards.check({record["id"]: record})
        self.assertEqual(len(errors), 1)
        self.assertIn("missing.py", errors[0])
        (self.root / "missing.py").touch()
        self.assertEqual(cards.check({record["id"]: record}), [])

    def test_duplicate_issue_and_obsolete_metadata(self):
        first = self.record()
        second = self.record("OTHER", phase="build")
        errors = cards.check({first["id"]: first, second["id"]: second})
        self.assertTrue(any("already has card" in error for error in errors))
        self.assertTrue(any("obsolete header field phase" in error for error in errors))

    def test_rejects_unclosed_private_block_and_duplicate_fields(self):
        path = self.card_dir / "BAD.md"
        for body in ("<!-- " "private -->\n---\nid: BAD\n---\n", "---\nid: BAD\nid: BAD\n---\n"):
            path.write_text(body)
            with self.assertRaises(ValueError):
                cards.read_card(path)

    def test_new_requires_live_issue_ownership(self):
        for state, assignees in (("CLOSED", [{"login": "sprucey149"}]), ("OPEN", [{"login": "contributor"}])):
            live = subprocess.CompletedProcess([], 0, json.dumps(dict(state=state, assignees=assignees)))
            with patch.object(cards.subprocess, "run", return_value=live), self.assertRaises(ValueError):
                cards.new_card("ISSUE-7", 7, "Scope")
        self.assertEqual(list(self.card_dir.glob("*.md")), [])

    def test_new_refuses_duplicate_issue_without_overwriting(self):
        live = subprocess.CompletedProcess([], 0, json.dumps(dict(state="OPEN", assignees=[{"login": "sprucey149"}])))
        with patch.object(cards.subprocess, "run", return_value=live) as command:
            path = cards.new_card("ISSUE-7", 7, "Scope")
            original = path.read_text()
            with self.assertRaises(ValueError):
                cards.new_card("ANOTHER", 7, "Other scope")
            self.assertEqual(command.call_count, 1)
            self.assertEqual(path.read_text(), original)
        self.assertEqual(cards.check(), [])

    def test_concurrent_creation_keeps_one_card_per_issue(self):
        ready = threading.Barrier(2)
        live = subprocess.CompletedProcess([], 0, json.dumps(dict(state="OPEN", assignees=[{"login": "sprucey149"}])))

        def query(*args, **kwargs):
            time.sleep(0.02)  # allow the other dispatcher to attempt creation
            return live

        def create(cid):
            ready.wait()
            try:
                cards.new_card(cid, 7, "Scope")
                return "created"
            except ValueError:
                return "duplicate"

        with patch.object(cards.subprocess, "run", side_effect=query), ThreadPoolExecutor(2) as pool:
            results = list(pool.map(create, ("FIRST", "SECOND")))
        self.assertCountEqual(results, ["created", "duplicate"])
        self.assertEqual(len(cards.cards()), 1)
        self.assertEqual(cards.check(), [])

    def test_new_rejects_malformed_header_values_before_github(self):
        with patch.object(cards.subprocess, "run") as command:
            for issue, title in ((0, "Scope"), (7, ""), (7, "Scope\nstatus: done")):
                with self.assertRaises(ValueError):
                    cards.new_card("ISSUE-7", issue, title)
            command.assert_not_called()


class AuditHolds(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "report.md").write_text("Unverified prerequisite checkpoint.\n")
        for name, value in (("LOOP", self.root), ("AUDIT_ROOT", self.root),
                            ("AUDIT_HOLDS", self.root / "holds.json"), ("JOURNAL", self.root / "journal.md")):
            patcher = patch.object(loopctl, name, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def checkpoint(self, missing):
        loopctl.audit_checkpoint("OLD", missing, "abc123", "inputs missing", "report.md")

    def test_checkpoint_retains_prior_prerequisites(self):
        self.checkpoint(["first"])
        self.checkpoint(["second", "second"])
        hold = loopctl.audit_holds()["OLD"]
        self.assertEqual(hold["missing"], ["first", "second"])
        self.assertIn("INCOMPLETE / UNAUDITED", loopctl.audit_hold_reason("OLD", {"OLD": hold}))

    def test_existing_inputs_and_absent_report_are_rejected(self):
        with self.assertRaises(ValueError):
            self.checkpoint(["report.md"])
        with self.assertRaises(ValueError):
            loopctl.audit_checkpoint("OLD", ["absent"], "abc123", "reason", "missing-report.md")
        self.assertEqual(loopctl.audit_holds(), {})

    def test_restored_inputs_do_not_delete_hold_or_earn_verdict(self):
        self.checkpoint(["first"])
        (self.root / "first").touch()
        with patch.object(sys, "argv", ["loopctl.py", "audit-status"]), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(loopctl.main(), 0)
        self.assertIn("inputs restored; remains UNAUDITED", output.getvalue())
        self.assertIn("OLD", loopctl.audit_holds())

    def test_reset_requires_review_and_records_reason(self):
        self.checkpoint(["first"])
        with self.assertRaises(ValueError):
            loopctl.audit_reset("OLD", "", "reason")
        self.assertIn("OLD", loopctl.audit_holds())
        loopctl.audit_reset("OLD", "maintainer", "scope revalidated")
        self.assertEqual(loopctl.audit_holds(), {})
        self.assertIn("reviewed by maintainer — scope revalidated", (self.root / "journal.md").read_text())

    def test_retired_scheduler_command_is_unavailable(self):
        with patch.object(sys, "argv", ["loopctl.py", "next"]), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as result:
                loopctl.main()
        self.assertEqual(result.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
