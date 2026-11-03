"""Guard and cleanup regressions for the issue-card migration."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import target_cleanup


class WorkflowToolsTests(unittest.TestCase):
    def guard(self, command):
        result = subprocess.run(
            ["bash", str(SCRIPTS / "claude_guard.sh")],
            input=json.dumps({"tool_name": "Bash", "tool_input": {"command": command}}),
            capture_output=True, text=True, check=True,
        )
        return result.stdout

    def test_owned_push_and_no_ff_merge_allowed(self):
        for command in ("git push origin my-branch", "git -C /repo merge --no-ff my-branch"):
            self.assertEqual(self.guard(command), "")

    def test_force_push_and_blanket_stage_denied(self):
        for command in ("git push --force origin main", "git -C /repo push origin +main",
                        "git push --force-with-lease origin main", "git add -A"):
            self.assertIn('"deny"', self.guard(command))

    def test_shared_inputs_are_protected(self):
        for path in ("iso", "extracted", "community_mods", "PC_swbf2_exe", "PC_swbf1_exe",
                     "engine/assets_baked", "engine/missions"):
            self.assertIn('"deny"', self.guard(f"touch ignored > {path}/file"))

    def test_private_card_protects_target(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            cards = root / "engine/artifacts/loop/cards/private"
            cards.mkdir(parents=True)
            (cards / "ISSUE-1.md").write_text(
                "<!-- " "private -->\n---\nid: ISSUE-1\nstatus: in-progress\nbranch: task\n---\n"
                "Evidence\n<!-- " "/private -->\n"
            )
            target = root / "task_target"
            target.mkdir()
            with patch.object(target_cleanup, "ROOT", temp):
                records = target_cleanup.cards()
            self.assertIn("ISSUE-1", records)
            self.assertEqual(target_cleanup.target_verdicts(temp, records, set()),
                             [(str(target), False, "live branch/worktree or open card")])


if __name__ == "__main__":
    unittest.main()
