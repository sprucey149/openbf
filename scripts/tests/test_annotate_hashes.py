"""HASHN fixture controls; real-property corpus acceptance remains separate."""
import contextlib
import io
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import annotate_hashes as ann
import re_check


class Annotate(unittest.TestCase):
    def test_known_hash_and_byte_folding(self):
        self.assertEqual(ann.fnv('GeometryName'), 0x47c86b4a)
        self.assertEqual(ann.fnv('GEOMETRYNAME'), 0x47c86b4a)
        self.assertEqual(ann.fnv('GeometryLowRes'), 0xa490eba1)
        self.assertEqual(ann.fnv('@'), ann.fnv('`'))  # OR 0x20, not str.lower

    def test_forms_and_line_preservation(self):
        # Fixed constants from RECHK's existing reader controls, not generated
        # by the annotator's hash implementation.
        text = ('0xa490eba1 -0x5b6f145f 2760960929 -1534006367 '
                '0xffffffffa490eba1ULL\n0x47c86b4a\n')
        table, _ = ann.name_table(['GeometryLowRes', 'GeometryName'])
        result, counts = ann.annotate(text, table)
        self.assertEqual(counts, dict(hex=2, negative_hex=1, unsigned_decimal=1,
                                      signed_decimal=1, sign_extended=1, pointer=0))
        self.assertEqual(result.count('FNV:'), 6)
        self.assertEqual(result.count('\n'), text.count('\n'))
        self.assertEqual(ann.MARKER.sub('', result), text)

    def test_false_positive_controls(self):
        words = (0x1234, *ann.NON_HASH_WORDS)
        table = {w: ['fake'] for w in words}
        text = ' '.join(hex(w) for w in words)
        self.assertEqual(ann.annotate(text, table)[0], text)
        text = ('/* 0xa490eba1 */ // 0xa490eba1\n"0xa490eba1" '
                "'1' DAT_a490eba1 FUN_12345678 abc2760960929 "
                '2760960929.0 2760960929e0 02760960929 0x12345678a490eba1 '
                '0xffffffff47c86b4a -2760960929')
        table, _ = ann.name_table(['GeometryLowRes', 'GeometryName'])
        self.assertEqual(ann.annotate(text, table)[0], text)

    def test_pointer_form(self):
        # Ghidra prints RollIn (0x1ce8c2e1) as an address: B1 line 32629 of the
        # real corpus compares a stack array element with `&DAT_1ce8c2e1`.
        table, _ = ann.name_table(['RollIn', 'GeometryName'])
        text = 'a == &DAT_1ce8c2e1 && b == &UNK_1ce8c2e1\n'
        result, counts = ann.annotate(text, table)
        self.assertEqual(counts['pointer'], 2)
        self.assertEqual(result.count('["RollIn"]'), 2)
        self.assertEqual(ann.MARKER.sub('', result), text)
        # A plain DAT_ is a memory read, and an address in main RAM is data.
        table[0x00675544] = ['fake']
        for text in ('x = DAT_1ce8c2e1;', 'p = &DAT_00675544;', 'p = &DAT_1ce8c2e1x;'):
            self.assertEqual(ann.annotate(text, table)[0], text)

    def test_explicit_non_hash_word(self):
        result, counts = ann.annotate('0x47c86b4a', {0x47c86b4a: ['GeometryName']},
                                      ann.NON_HASH_WORDS | {0x47c86b4a})
        self.assertEqual(result, '0x47c86b4a')
        self.assertFalse(any(counts.values()))

    def test_collision_and_alias_counts(self):
        self.assertEqual(ann.fnv('costarring'), ann.fnv('liquid'))
        table, stats = ann.name_table(['costarring', 'liquid', 'MaxSpeed', 'MAXSPEED'])
        self.assertEqual(stats, dict(collision_buckets=1, colliding_names=2))
        result, _ = ann.annotate(hex(ann.fnv('liquid')), table)
        self.assertIn('["costarring", "liquid"]', result)
        _, stats = ann.name_table(['MaxSpeed', 'MAXSPEED'])
        self.assertEqual(stats['collision_buckets'], 0)

    def test_unsafe_name(self):
        with self.assertRaises(ValueError):
            ann.name_table(['unsafe*/name'])

    def test_provenance_and_query(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            corpus = {'b1': 'a-decomp.c', 'b2': 'b-decomp.c'}
            table, _ = ann.name_table(['GeometryName'])
            source = ('/* ==== 00100000 FUN_a ==== */\n'
                      'x = 0x47c86b4a;\n/* ==== 00100010 FUN_b ==== */\n'
                      'x = 0x47c86b4a; y = 0x47c86b4a;\n')
            for path in corpus.values():
                (root / path).write_text(source)
                ann.write_corpus(root / path, table)
            with patch.object(re_check, 'ROOT', directory), patch.object(re_check, 'CORP', corpus), \
                    patch.object(re_check, '_cache', {}):
                out = io.StringIO()
                with contextlib.redirect_stdout(out):
                    re_check.hash_name('geometryname')
                for game in ('B1', 'B2'):
                    self.assertIn(f'{game}:2 is in FUN_a', out.getvalue())
                    self.assertIn(f'{game}:4 is in FUN_b', out.getvalue())
                self.assertIn('2 annotated line(s) in 2 function(s)', out.getvalue())
                self.assertEqual((root / corpus['b1']).read_text(), source)
                # No partial B1 result when B2's source changes after annotation.
                (root / corpus['b2']).write_text(source + 'changed\n')
                out = io.StringIO()
                with contextlib.redirect_stdout(out), self.assertRaises(ValueError):
                    re_check.hash_name('GeometryName')
                self.assertEqual(out.getvalue(), '')
                ann.write_corpus(root / corpus['b2'], table)
                target = ann.annotated_path(root / corpus['b1'])
                target.write_text(target.read_text() + 'tampered\n')
                with self.assertRaises(ValueError):
                    re_check.hash_name('GeometryName')

    def test_cli_both_games_and_missing_input(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'analysis').mkdir()
            names = root / 'names.txt'
            names.write_text('GeometryName = "GeometryLowRes"\n')
            for elf in ann.CORPORA:
                (root / 'analysis' / f'{elf}-decomp.c').write_text('x = 0x47c86b4a;\n')
            cmd = [sys.executable, ann.__file__, '--root', directory, '--names', str(names)]
            run = subprocess.run(cmd, capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(run.stdout.count('"hex": 1'), 2)
            for elf in ann.CORPORA:
                self.assertIn('GeometryName', (root / 'analysis' / f'{elf}-decomp.annotated.c').read_text())
            names.write_bytes(b'binary\0name')
            run = subprocess.run(cmd, capture_output=True, text=True)
            self.assertEqual(run.returncode, 2)
            self.assertIn('binary name source', run.stderr)
            names.write_text('GeometryName')
            (root / 'analysis' / f'{ann.CORPORA[1]}-decomp.c').unlink()
            run = subprocess.run(cmd, capture_output=True, text=True)
            self.assertEqual(run.returncode, 2)
            self.assertIn('corpus unavailable', run.stderr)



# Each property's functions in the real corpora, checked independently against the
# ELFs (lui + addiu/ori immediate pairs mapped through <ELF>-functions.txt).
KNOWN_READERS = {
    'MaxSpeed': {'b1': {'001cb768', '001d3f68', '001df3f0', '0020f2e0', '0021d980'},
                 'b2': {'001d1f58', '001f12c0', '001fd2f8', '0020fd58', '00252bb0', '00263f68'}},
    'SkeletonRootScale': {'b1': {'0020f2e0'}, 'b2': {'00252bb0', '00263f68'}},
    'ChunkFrequency': {'b1': {'0020f2e0'}, 'b2': {'00252bb0'}},
    'FirePointName': {'b1': {'0026e260', '002864f0'}, 'b2': {'00302f10', '00328fb0', '00342528'}},
    'TriggerRadius': {'b1': {'001f96b0', '00214140'}, 'b2': {'0022c7a8', '002586e0'}},
}


# Captured at import: other suites patch re_check's globals while they run.
REAL_ROOT, REAL_CORP = Path(re_check.ROOT), dict(re_check.CORP)


@unittest.skipUnless(all((REAL_ROOT / path).is_file() for path in REAL_CORP.values()),
                     'real decomp corpora not restored (analysis/ is local-only)')
class RealCorpus(unittest.TestCase):
    def test_five_known_properties(self):
        table, _ = ann.name_table(list(KNOWN_READERS))
        with tempfile.TemporaryDirectory() as directory:
            for path in REAL_CORP.values():
                link = Path(directory) / path
                link.parent.mkdir(parents=True, exist_ok=True)
                link.symlink_to(REAL_ROOT / path)
                ann.write_corpus(link, table)
            with patch.object(re_check, 'ROOT', directory), patch.object(re_check, 'CORP', REAL_CORP), \
                    patch.object(re_check, '_cache', {}):
                for name, games in KNOWN_READERS.items():
                    out = io.StringIO()
                    with contextlib.redirect_stdout(out):
                        re_check.hash_name(name)
                    for game, expected in games.items():
                        found = set(re.findall(rf'^{game.upper()}:\d+ is in \S+ \(0x([0-9a-f]{{8}})',
                                               out.getvalue(), re.M))
                        self.assertEqual(found, expected, (name, game))


if __name__ == '__main__':
    unittest.main()
