"""Packed Windows executable boundary; no game data or large allocation needed."""
from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import pack


class PackLimitTests(unittest.TestCase):
    def test_windows_limit_is_strictly_below_four_gib(self):
        with mock.patch.object(pack, 'WINDOWS', True), mock.patch.object(pack.os.path, 'getsize') as size:
            size.return_value = 2**32 - 1
            pack.check_packed_size('output.new')
            for length in (2**32, 2**32 + 1):
                size.return_value = length
                with self.assertRaisesRegex(SystemExit, '4 GiB'):
                    pack.check_packed_size('output.new')

    def test_non_windows_keeps_large_file_support(self):
        with mock.patch.object(pack, 'WINDOWS', False), mock.patch.object(pack.os.path, 'getsize') as size:
            pack.check_packed_size('output.new')
            size.assert_not_called()


if __name__ == '__main__':
    unittest.main()
