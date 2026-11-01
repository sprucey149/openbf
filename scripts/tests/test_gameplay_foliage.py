"""Address-resolution and rejection tests for the foliage data generator."""
import importlib.util
from pathlib import Path
import struct
import unittest

SPEC = importlib.util.spec_from_file_location(
    "gameplay_foliage", Path(__file__).resolve().parents[1] / "gameplay_foliage.py")
foliage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(foliage)


def executable(payload=b"lookup", address=0x1000):
    result = bytearray(84)
    result[:7] = b"\x7fELF\x01\x01\x01"
    struct.pack_into("<H", result, 18, 8)
    struct.pack_into("<I", result, 28, 52)
    struct.pack_into("<HH", result, 42, 32, 1)
    struct.pack_into("<IIIII", result, 52, 1, 84, address, address, len(payload))
    return bytes(result) + payload


class FoliageTests(unittest.TestCase):
    def test_virtual_range_uses_load_offset(self):
        self.assertEqual(foliage.elf_bytes(executable(), 0x1001, 3), b"ook")

    def test_refuses_non_elf_and_wrong_architecture(self):
        for source in (b"", b"MZ" + bytes(90)):
            with self.assertRaisesRegex(ValueError, "ELF32"):
                foliage.elf_bytes(source, 0, 1)
        source = bytearray(executable())
        struct.pack_into("<H", source, 18, 62)
        with self.assertRaisesRegex(ValueError, "MIPS"):
            foliage.elf_bytes(source, 0x1000, 1)

    def test_refuses_outside_load_range(self):
        with self.assertRaisesRegex(ValueError, "one LOAD"):
            foliage.elf_bytes(executable(), 0x1005, 2)

    def test_refuses_short_headers_and_payload(self):
        with self.assertRaisesRegex(ValueError, "header table"):
            foliage.elf_bytes(executable()[:75], 0x1000, 1)
        with self.assertRaisesRegex(ValueError, "beyond executable"):
            foliage.elf_bytes(executable()[:-2], 0x1000, 6)

    def test_checksum_rejects_unknown_table(self):
        first, second = foliage.ADDRESSES["bf1"]
        low = min(first, second)
        source = executable(bytes(max(first + 1024, second + 0x110) - low), low)
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            foliage.extract(source, "bf1")

    def test_fnv_known_vector(self):
        self.assertEqual(foliage.fnv(b"hello"), 0x4F9F2CAB)


if __name__ == "__main__":
    unittest.main()
