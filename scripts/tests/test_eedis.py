"""eedis.py reads only bytes the ELF really holds, so BSS never prints as a run of nops."""
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import eedis
import elfrd


class Word(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        header = bytearray(0x54)
        header[:4] = b"\x7fELF"
        struct.pack_into("<I", header, 0x1c, 0x34)
        struct.pack_into("<HH", header, 0x2a, 0x20, 1)
        # One PT_LOAD at 0x1000: 8 bytes in the file, 16 in memory.
        struct.pack_into("<8I", header, 0x34, 1, 0x54, 0x1000, 0x1000, 8, 16, 7, 4)
        path = Path(directory.name) / "t.elf"
        path.write_bytes(bytes(header) + struct.pack("<2I", 0x03e00008, 0x0000282d))
        self.elf = elfrd.Elf(path)

    def test_reads_file_backed_words(self):
        self.assertEqual(eedis.word(self.elf, 0x1000), 0x03e00008)
        self.assertEqual(eedis.word(self.elf, 0x1004), 0x0000282d)

    def test_refuses_bss_and_addresses_outside_the_elf_by_name(self):
        for va in (0x1008, 0x100c, 0x1010, 0xffc):
            with self.assertRaisesRegex(ValueError, hex(va)):
                eedis.word(self.elf, va)

    def test_refuses_a_word_past_the_end_of_the_file_bytes(self):
        with self.assertRaisesRegex(ValueError, "0x1006"):
            eedis.word(self.elf, 0x1006)


class Index(unittest.TestCase):
    def test_scripts_readme_lists_every_entry(self):
        scripts = Path(__file__).resolve().parents[1]
        text = (scripts / "README.md").read_text()
        ignored = {"README.md", "__pycache__"}
        missing = [p.name for p in scripts.iterdir() if p.name not in ignored and f"`{p.name}" not in text]
        self.assertEqual(missing, [])


if __name__ == "__main__":
    unittest.main()
