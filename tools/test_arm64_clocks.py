"""tools/arm64/clocks.py: modelled clocks must follow the instruction latencies of the model it asks."""

import os
import re
import subprocess
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "arm64"))
import clocks  # noqa: E402
import count  # noqa: E402

HAVE_MCA = os.path.exists(f"{clocks.LLVM}/llvm-mca")


def latency(text):
    """The latency llvm-mca's own `-instruction-info` prints for the one instruction `text`."""
    out = subprocess.run([f"{clocks.LLVM}/llvm-mca", "-mtriple=aarch64-linux-gnu", "-mcpu=apple-m1", "-iterations=1", "-instruction-info", "-"], input=text, capture_output=True, text=True).stdout
    return int(re.search(r"^\s*\d+\s+(\d+)\s+[\d.]+\s+.*" + re.escape(text.split()[0]), out, re.M).group(1))


class Blocks(unittest.TestCase):
    CODE = {0x100: "mov x0, #0", 0x104: "add x0, x0, #1", 0x108: "cmp x0, #9", 0x10C: "b.ne 0x104 <f+0x4>", 0x110: "ret"}

    def test_a_loop_body_is_one_block_cut_at_its_branch_target(self):
        # Cutting only after branches priced the pre-header and the loop body as one block of 4 instructions run 10 times.
        executed = {0x100: 1, 0x104: 10, 0x108: 10, 0x10C: 10, 0x110: 1}
        self.assertEqual(clocks.blocks(self.CODE, executed), [[0x100], [0x104, 0x108, 0x10C], [0x110]])


class Range(unittest.TestCase):
    def test_the_range_is_chosen_with_kernel_or_whole(self):
        # The docstring promised `ELF [--kernel]` and the parser rejected it ("unrecognized arguments"), so the 11
        # LVI cells were priced whole-program: runtime included, a 24% instruction cut read as 1.8% of the clocks.
        self.assertTrue(clocks.parser().parse_args(["a.elf", "--kernel"]).kernel)
        self.assertTrue(clocks.parser().parse_args(["a.elf", "--whole"]).whole)

    def test_no_range_is_an_error_not_a_default(self):
        # count.py's rule: a count with no flag once ran whole and was read as the kernel.
        with self.assertRaises(SystemExit):
            clocks.parser().parse_args(["a.elf"])

    def test_the_kernel_histogram_leaves_out_main_and_the_runtime(self):
        # Priced from a histogram that held every address, --kernel would still have priced the runtime.
        self.assertTrue(count.histogram_wants("kernel", kernel=True))
        self.assertFalse(count.histogram_wants("main", kernel=True))
        self.assertFalse(count.histogram_wants("runtime", kernel=True))
        self.assertTrue(all(count.histogram_wants(name, kernel=False) for name in ("kernel", "main", "runtime")))


@unittest.skipUnless(HAVE_MCA, "needs the pin's llvm-mca (LLVM_BIN)")
class Model(unittest.TestCase):
    def test_a_dependent_chain_costs_the_model_latency_per_instruction(self):
        # The by-hand check: 4 chained muls cost 4 x the latency the model prints for one, in steady state.
        self.assertAlmostEqual(clocks.mca_cycles(["mul x0, x0, x0"] * 4, 100), 4 * latency("mul x0, x0, x0"), delta=0.2)

    def test_independent_instructions_overlap(self):
        chain = clocks.mca_cycles(["mul x0, x0, x0"] * 4, 100)
        independent = clocks.mca_cycles(["mul x0, x1, x1", "mul x2, x3, x3", "mul x4, x5, x5", "mul x6, x7, x7"], 100)
        self.assertLess(independent * 3, chain)

    def test_a_self_loop_is_priced_per_iteration_times_its_count(self):
        code = {0x100: "mul x0, x0, x0", 0x104: "mul x0, x0, x0", 0x108: "b.ne 0x100 <f>"}
        clocks_, instructions = clocks.modelled_clocks(code, {0x100: 10, 0x104: 10, 0x108: 10})
        self.assertEqual(instructions, 30)
        self.assertAlmostEqual(clocks_, 10 * clocks.mca_cycles(["mul x0, x0, x0", "mul x0, x0, x0", "b.ne 0x100 <f>"], 100), delta=1)

    def test_a_call_costs_a_branch_not_the_hundred_clocks_llvm_mca_assumes(self):
        # llvm-mca assumes 100 cycles for any `bl`/`blr` ("call instructions are not correctly modeled"): fib -O2 was
        # 64,037 modelled clocks for 7,653 instructions, 8.4 a instruction, every call block priced at 103.
        code = {0x100: "mov x21, x0", 0x104: "bl 0x200 <g>"}
        clocks_, instructions = clocks.modelled_clocks(code, {0x100: 1, 0x104: 1})
        self.assertEqual(instructions, 2)
        self.assertLess(clocks_, 10)


if __name__ == "__main__":
    unittest.main()
