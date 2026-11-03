"""The checker may not accept early rows after a rebake changed their input."""
import copy
from pathlib import Path
import tempfile
import sys
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import render_input_stability as s


class InputTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.reference=self.root/'ref';self.source=self.root/'src'
        for root in (self.reference,self.source):
            root.mkdir();(root/'manifest.ron').write_text('control')
        self.cases=[('source',self.source)]
        self.program=self.root/'program.py';self.program.write_text('measuring program')
        self.mock=patch.object(s,'source_snapshot',side_effect=self.program_snapshot)
        self.mock.start();self.addCleanup(self.mock.stop)
    def program_snapshot(self):
        from world_fingerprint import digest
        row=s.file_record(self.program,'program.py');rows=[row]
        parts={'scripts':dict(files=rows,directory_links=[],sha256=digest([rows,[]]))}
        return dict(parts=parts,sha256=digest(parts),content_sha256=digest([['scripts',row['path'],row['sha256']]]))
    def capture(self):return s.capture(self.reference,self.cases)
    def receipt(self):return s.finish(self.capture(),self.reference,self.cases)
    def test_unchanged_content_stats_roots_and_source_pass(self):
        self.assertEqual(s.problems(self.receipt(),self.reference,self.cases),[])
    def test_public_report_validator_passes_cli_cases_to_real_input_validator(self):
        import render_collision_check as check
        report=dict(schema=check.SCHEMA,complete=True,method=check.METHOD,
                    tolerance_m=.01,margin_reason=check.MARGIN_REASON,
                    reference=str(self.reference),maps=[],input_stability=self.receipt())
        # Exercise the real boundary: the CLI/gate supplies NAME=PATH strings,
        # while content inventories use parsed (name, Path) pairs.
        self.assertEqual(check.report_problems(report,self.reference,[f'source={self.source}']),
                         ['report has no source/map rows'])
    def test_same_size_changed_content_cannot_reuse_early_rows(self):
        before=self.capture();(self.source/'manifest.ron').write_text('changed')
        receipt=s.finish(before,self.reference,self.cases)
        self.assertEqual(receipt['status'],'unmeasured')
        self.assertTrue(s.problems(receipt,self.reference,self.cases))
    def test_optional_clip_or_texture_addition_and_deletion_are_detected(self):
        for name in ('soldier.render-clips.glb','sheet.tier2.ktx2','sets.json'):
            before=self.capture();path=self.source/name;path.write_bytes(b'new dependency')
            self.assertEqual(s.finish(before,self.reference,self.cases)['status'],'unmeasured')
            before=self.capture();path.unlink()
            self.assertEqual(s.finish(before,self.reference,self.cases)['status'],'unmeasured')
    def test_current_files_must_still_equal_the_completed_receipt(self):
        receipt=self.receipt();(self.reference/'manifest.ron').write_text('later control')
        self.assertTrue(s.problems(receipt,self.reference,self.cases))
    def test_wrong_root_or_forged_complete_flag_refused(self):
        receipt=self.receipt();receipt['complete']='true'
        self.assertTrue(s.problems(receipt,self.reference,self.cases))
        receipt=self.receipt();receipt['before']['roots']['reference']='/stale/root'
        self.assertTrue(s.problems(receipt,self.reference,self.cases))
    def test_symlinked_directories_and_optional_sidecars_are_inventoried(self):
        external=self.root/'shared';external.mkdir();(external/'body.glb').write_bytes(b'geometry')
        (self.source/'common').symlink_to(external,target_is_directory=True)
        before=self.capture();paths=[row['path'] for row in before['records']['case:source']['files']]
        self.assertIn('common/body.glb',paths)
        (external/'body.render-clips.glb').write_bytes(b'pose')
        self.assertEqual(s.finish(before,self.reference,self.cases)['status'],'unmeasured')
    def test_directory_cycles_and_missing_roots_fail_closed(self):
        (self.source/'cycle').symlink_to(self.source,target_is_directory=True)
        self.assertTrue(self.capture()['errors'])
        self.assertTrue(s.capture(self.root/'missing',self.cases)['errors'])
    def test_measuring_source_changes_invalidate_the_receipt(self):
        before=self.capture();self.program.write_text('changed program')
        self.assertEqual(s.finish(before,self.reference,self.cases)['status'],'unmeasured')
        receipt=self.receipt();self.program.write_text('changed again')
        self.assertTrue(s.problems(receipt,self.reference,self.cases))
    def test_malformed_stat_boolean_and_catalog_deletion_refused(self):
        from world_fingerprint import digest
        receipt=self.receipt()
        for side in ('before','after'):
            row=receipt[side]['records']['reference'];row['files'][0]['stat'][0]=True
            row['sha256']=digest([row['files'],row['directory_links']])
        self.assertTrue(s.problems(receipt,self.reference,self.cases))
        receipt=self.receipt()
        for side in ('before','after'):receipt[side]['records'].pop('case:source')
        self.assertTrue(s.problems(receipt,self.reference,self.cases))


if __name__=='__main__':unittest.main()
