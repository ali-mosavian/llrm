"""The pin's MIR text is read to the opcodes per block, including one-letter ones."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from arm64_shadow import PIN_FLAGS, OracleFailed, compare, ours, parse_pin, pin  # noqa: E402

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




class PinCpu(unittest.TestCase):
    def test_the_pin_runs_for_the_cpu_of_the_table(self):
        toml = (Path(__file__).resolve().parents[2] / "crates/target/llrm-arm64/dagisel.toml").read_text()
        self.assertIn('cpus = ["apple-m1"]', toml)
        self.assertIn("-mcpu=apple-m1", PIN_FLAGS)


class Compare(unittest.TestCase):
    def test_one_seeded_wrong_opcode_is_the_only_function_flagged(self):
        # The control: of three functions only the one whose opcode is wrong may be a `differ`.
        ours_ = {"a": [["ADDWri", "B"]], "b": [["SUBWri"]], "c": [["MOVi32imm", "RET_ReallyLR"]]}
        pin_ = {"a": [["ADDWri", "B"]], "b": [["ADDWri"]], "c": [["MOVi32imm", "RET_ReallyLR"]]}
        flagged = [line for line in compare("f.ll", ours_, {}, pin_) if "differ" in line]
        self.assertEqual(len(flagged), 1, flagged)
        self.assertIn("@b:", flagged[0])

    def test_an_oracle_failure_is_a_line_not_an_abort(self):
        # `sys.exit` on a module the pin's llc could not parse ended the whole 92-file run after 15 functions.
        with self.assertRaises(OracleFailed):
            pin("/bin/false", "define void @f() { ret void }")


if __name__ == "__main__":
    unittest.main()
