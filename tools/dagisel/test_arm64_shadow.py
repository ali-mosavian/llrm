"""The pin's MIR text is read to the opcodes per block, including one-letter ones."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from arm64_shadow import ours, parse_pin  # noqa: E402

MIR = """name:            f
body:             |
  bb.0:
    successors: %bb.1(0x80000000)
    %0:gpr32 = COPY $w0
    %1:gpr32 = ADDWri %0, 1, 0
    B %bb.1

  bb.1:
    $w0 = COPY %1
    RET_ReallyLR implicit $w0
"""


class ParsePin(unittest.TestCase):
    def test_an_unconditional_branch_is_counted(self):
        # `B` is one letter: the reader dropped it and every function with a jump looked different.
        self.assertEqual(parse_pin(MIR), {"f": [["ADDWri", "B"], ["RET_ReallyLR"]]})


class PinModule(unittest.TestCase):
    def test_the_pin_gets_the_module_before_our_cgpaddr(self):
        # After cgpaddr the `ret` is already copied into the tail-call block; the pin's own CodeGenPrepare
        # then merged the rets first and block order read [RET][TCRETURN]: 6 rows differed for nothing.
        corpus = Path(__file__).resolve().parents[2] / "crates/target/llrm-arm64/tests/data/conditional_compare_branch.ll"
        module, _, _ = ours(str(corpus))
        self.assertNotIn("ret void\n  tail call", module)
        self.assertEqual(module.count("ret void"), 1, module)


if __name__ == "__main__":
    unittest.main()
