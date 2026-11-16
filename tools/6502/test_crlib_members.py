"""The float and double to u64 conversions are one asm member at the 6502 (config/6502/fixunsdi.s): the C members were 181 and 220 B, each with a
64-bit call, and double_to_uint64 -O1 was 1,174 B against llvm-mos 1,061 (894 B with the asm; 2026-11-16)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import crlib  # noqa: E402

RT = Path(__file__).resolve().parents[2] / "runtime" / "compiler-rt"


class Members(unittest.TestCase):
    def test_the_u64_conversions_resolve_to_the_asm_member(self):
        index = crlib.index(RT)
        for symbol in ("__fixunssfdi", "__fixunsdfdi"):
            self.assertEqual(index[symbol], "config/6502/fixunsdi.s", symbol)

    def test_the_shifts_export_the_entries_the_conversions_jump_to(self):
        index = crlib.index(RT)
        self.assertEqual(index["__ashldi3_a"], "config/6502/ashldi3.s")
        self.assertEqual(index["__lshrdi3_a"], "config/6502/lshrdi3.s")


if __name__ == "__main__":
    unittest.main()
