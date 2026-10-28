import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import ehcheck

DUMP = """00000018 0000000000000024 0000001c FDE cie=00000000 pc=0000000000000010..0000000000000046
  DW_CFA_advance_loc: 1 to 0000000000000011
  DW_CFA_def_cfa_offset: 16
  DW_CFA_offset: r6 (rbp) at cfa-16
  DW_CFA_advance_loc: 48 to 0000000000000041
  DW_CFA_def_cfa_offset: 8
  DW_CFA_nop
"""


class ParseTest(unittest.TestCase):
    def test_a_function_is_its_frame_changes_without_advances_or_padding(self):
        """The advances differ whenever the code does; comparing them reported every function as different."""
        self.assertEqual(
            ehcheck.parse(DUMP, {0x10: "f"}),
            {"f": ["DW_CFA_def_cfa_offset: 16", "DW_CFA_offset: r6 (rbp) at cfa-16", "DW_CFA_def_cfa_offset: 8"]},
        )


if __name__ == "__main__":
    unittest.main()
