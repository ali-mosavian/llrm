"""check_pats.py names a Pat that llvm-tblgen drops without a word. Runs under unittest and pytest."""

import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_pats  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
BUILD = Path(os.environ.get("LLVM_BUILD", Path.home() / "scratch/llvm-build-m68k"))
SRC = Path(os.environ.get("LLVM_SRC", Path.home() / "scratch/llvm-src")) / "llvm"
TBLGEN = [str(BUILD / "bin/llvm-tblgen"), f"-I{BUILD}/include", f"-I{SRC}/include", f"-I{SRC}/lib/Target", "-omit-comments"]

UNTYPED = "(REG_SEQUENCE GR16, (LD8ri (i8 0)), sub_lo, (EXTRACT_SUBREG GR16:$x, sub_lo), sub_hi)"
TYPED = "(REG_SEQUENCE GR16, (i8 (LD8ri (i8 0))), sub_lo, (i8 (EXTRACT_SUBREG GR16:$x, sub_lo)), sub_hi)"


@unittest.skipUnless((BUILD / "bin/llvm-tblgen").exists(), "no llvm-tblgen build")
class CheckPats(unittest.TestCase):
    def flagged(self, result):
        with tempfile.TemporaryDirectory() as work:
            td = Path(work) / "z80.td"
            shutil.copy(ROOT / "crates/target/llrm-z80/z80.td", td)
            td.write_text(td.read_text() + f"\ndef : Pat<(i16 (shl GR16:$x, (i8 7))), {result}>;\n")
            return check_pats.check(td, [ROOT / "crates/target/llrm-z80"], TBLGEN)

    def test_a_pat_with_an_untyped_nested_result_is_named(self):
        """It made the same table with and without it: crc16 kept a shift and two xors a Pat was written to remove."""
        found = self.flagged(UNTYPED)
        self.assertTrue(len(found) == 1 and "REG_SEQUENCE" in found[0] and "i8 7" in found[0], found)

    def test_the_typed_pat_has_a_matcher_entry(self):
        self.assertEqual(self.flagged(TYPED), [])


if __name__ == "__main__":
    unittest.main()
