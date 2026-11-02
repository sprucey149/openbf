"""The alignment gate must distrust a zero exit without a complete receipt."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gates
from render_collision_check import REQUIRED_MAPS


class AlignmentReceipt(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.g = object.__new__(gates.Gates)
        self.g.tree = str(self.root)
        self.g.out = str(self.root)
        self.g.env = {}
        self.g.rows = []
        self.g.alignment_reference = str(self.root / 'reference')
        self.g.alignment_cases = [f'xbox={self.root / "source"}']
        self.receipt = dict(schema=1, complete=True, reference=self.g.alignment_reference,
                            maps=[dict(source='xbox', map=key, placements=1, extra_source_placements=0,
                                       counts={'pass': 1}, objects=[{'status': 'pass'}]) for key in REQUIRED_MAPS])

    def execute(self, receipt=None, raw=None):
        def zero_helper(*_args, **_kwargs):
            if receipt is not None:
                (self.root / 'alignment.json').write_text(json.dumps(receipt))
            elif raw is not None:
                (self.root / 'alignment.json').write_text(raw)
            return 0
        with patch.object(gates, 'sh', side_effect=zero_helper):
            self.g.g_alignment()
        return self.g.rows[-1]['verdict']

    def test_zero_helper_without_output_fails(self):
        self.assertEqual(self.execute(), 'FAIL')

    def test_truncated_json_fails(self):
        self.assertEqual(self.execute(raw='{"complete":'), 'FAIL')

    def test_partial_report_fails(self):
        self.receipt['complete'] = False
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_empty_report_fails(self):
        self.receipt['maps'] = []
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_map_fails(self):
        self.receipt['maps'].pop()
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_duplicate_map_fails(self):
        self.receipt['maps'][-1] = copy.deepcopy(self.receipt['maps'][0])
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_extra_placements_fail(self):
        self.receipt['maps'][0]['extra_source_placements'] = 1
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_offender_fails_even_with_zero_exit(self):
        row = self.receipt['maps'][0]
        row.update(counts={'offender': 1}, objects=[{'status': 'offender'}])
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_unmeasured_fails_even_with_zero_exit(self):
        row = self.receipt['maps'][0]
        row.update(counts={'unmeasured': 1}, objects=[{'status': 'unmeasured'}])
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_inconsistent_counts_fail(self):
        self.receipt['maps'][0]['objects'][0]['status'] = 'offender'
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_visual_only_map_cannot_substitute_for_a_surface_measurement(self):
        row = self.receipt['maps'][0]
        row.update(counts={'no_render_geometry': 1}, objects=[{'status': 'no_render_geometry'}])
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_complete_clean_report_passes(self):
        self.assertEqual(self.execute(self.receipt), 'PASS')
