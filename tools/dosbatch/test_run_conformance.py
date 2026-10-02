"""`uv run --project tools python -m unittest discover -s tools/dosbatch`"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import run_conformance  # noqa: E402


class CaseTests(unittest.TestCase):
    def test_a_case_with_a_companion_is_skipped_not_run_alone(self):
        self.assertEqual(run_conformance.skipped({"runtime": "required", "companions": ["X.BAS"]}), "has companions")

    def test_bcs_switches_become_llrms_flags(self):
        self.assertEqual(run_conformance.flags({"switches": ["/O", "/R", "/Ah"]}), ["--array-order", "row-major", "--huge-arrays"])

    def test_expected_names_a_file_whatever_its_case(self):
        """PDHUGE.OUT was read as the literal text 'PDHUGE.OUT' (the file is pdhuge.out): three passing programs failed."""
        directory = Path(__file__).resolve().parents[2] / "tests/differential/conformance/pds71"
        self.assertNotEqual(run_conformance.expected(directory, {"expected": "PDHUGE.OUT"}), "PDHUGE.OUT")


if __name__ == "__main__":
    unittest.main()
