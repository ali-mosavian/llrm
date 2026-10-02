"""`uv run --project tools python -m unittest discover -s tools/dosbatch`"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402


class SourceTests(unittest.TestCase):
    def test_a_source_for_bc_ends_its_lines_with_cr_lf(self):
        """BC read a LF-only file as one line, so a leading comment ate the program:
        every BC run printed nothing, and the two programs expected to differ from BC passed."""
        self.assertEqual(dosbatch.crlf(b"' c\nPRINT 1\r\nEND\n"), b"' c\r\nPRINT 1\r\nEND\r\n")


if __name__ == "__main__":
    unittest.main()
