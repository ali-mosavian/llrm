"""tools/arm64/clocks.py: modelled clocks must follow the instruction latencies of the model it asks."""

import os
import re
import subprocess
import sys
import unittest
import unittest.mock

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "arm64"))
import clocks  # noqa: E402
import count  # noqa: E402



def latency(text):
    """The latency llvm-mca's own `-instruction-info` prints for the one instruction `text`."""
    out = subprocess.run([f"{clocks.LLVM}/llvm-mca", "-mtriple=aarch64-linux-gnu", "-mcpu=apple-m1", "-iterations=1", "-instruction-info", "-"], input=text, capture_output=True, text=True).stdout
    return int(re.search(r"^\s*\d+\s+(\d+)\s+[\d.]+\s+.*" + re.escape(text.split()[0]), out, re.M).group(1))


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

    def test_the_kernel_trace_leaves_out_main_and_the_runtime(self):
        # Priced from a trace that held every address, --kernel would still have priced the runtime.
        self.assertTrue(count.trace_wants("kernel", kernel=True))
        self.assertFalse(count.trace_wants("main", kernel=True))
        self.assertFalse(count.trace_wants("runtime", kernel=True))
        self.assertTrue(all(count.trace_wants(name, kernel=False) for name in ("kernel", "main", "runtime")))


class Tools(unittest.TestCase):
    def python(self, llvm_bin, *args):
        env = {k: v for k, v in os.environ.items() if k != "LLVM_BIN"}
        if llvm_bin:
            env["LLVM_BIN"] = llvm_bin
        return subprocess.run([sys.executable, *args], capture_output=True, text=True, env=env, cwd=os.path.dirname(__file__))

    def test_the_default_directory_has_the_tools_the_model_runs(self):
        # The default was a build directory that a cleanup emptied of llvm-mca: the tool could not price anything.
        run = self.python(None, "-c", "import sys; sys.path.insert(0, 'arm64'); import clocks; print(clocks.LLVM)")
        for tool in ("llvm-mca", "llvm-objdump"):
            self.assertTrue(os.path.exists(f"{run.stdout.strip()}/{tool}"), f"{run.stdout.strip()}/{tool}")

    def test_a_missing_llvm_mca_fails_the_model_tests(self):
        # They were skipped instead, so the file passed with the model untested.
        run = self.python("/nonexistent", "-m", "unittest", "test_arm64_clocks.Model")
        self.assertNotEqual(run.returncode, 0, run.stderr)


class Model(unittest.TestCase):
    def test_a_dependent_chain_costs_the_model_latency_per_instruction(self):
        # The by-hand check: 4 chained muls cost 4 x the latency the model prints for one, in steady state.
        self.assertAlmostEqual(clocks.mca_cycles(["mul x0, x0, x0"] * 4, 100), 4 * latency("mul x0, x0, x0"), delta=0.2)

    def test_independent_instructions_overlap(self):
        chain = clocks.mca_cycles(["mul x0, x0, x0"] * 4, 100)
        independent = clocks.mca_cycles(["mul x0, x1, x1", "mul x2, x3, x3", "mul x4, x5, x5", "mul x6, x7, x7"], 100)
        self.assertLess(independent * 3, chain)

    def test_a_loop_costs_its_steady_state_and_one_pipeline_fill(self):
        code = {0x100: "mul x0, x0, x0", 0x104: "mul x0, x0, x0", 0x108: "b.ne 0x100 <f>"}
        clocks_, instructions = clocks.modelled_clocks(code, [0x100, 0x104, 0x108] * 10)
        self.assertEqual(instructions, 30)
        self.assertAlmostEqual(clocks_, 10 * clocks.mca_cycles(["mul x0, x0, x0", "mul x0, x0, x0", "b.ne 0x100 <f>"], 100), delta=5)

    # A loop of two blocks, the first leaving over code that never ran to reach the second.
    SPLIT = {0x100: "fadd s0, s0, s1", 0x104: "cmp x1, x2", 0x108: "b.eq 0x110 <f>", 0x110: "subs x0, x0, #1", 0x114: "b.ne 0x100 <f>"}

    def test_a_loop_of_several_blocks_overlaps_its_trips(self):
        # Each block was priced as one cold run per execution: such a loop read 8 times its cost and a branchless
        # build of it read faster than a branching one that the core runs quicker.
        trace = list(self.SPLIT) * 400
        steady = 400 * clocks.mca_cycles(list(self.SPLIT.values()), 400)
        clocks_, instructions = clocks.modelled_clocks(self.SPLIT, trace)
        self.assertEqual(instructions, len(trace))
        self.assertAlmostEqual(clocks_, steady, delta=0.02 * steady)

    def test_a_long_trace_is_priced_in_windows_without_losing_any(self):
        trace = list(self.SPLIT) * 400
        whole, _ = clocks.modelled_clocks(self.SPLIT, trace)
        with unittest.mock.patch.object(clocks, "WINDOW", 503):
            parts, instructions = clocks.modelled_clocks(self.SPLIT, trace)
        self.assertEqual(instructions, len(trace))
        self.assertAlmostEqual(parts, whole, delta=0.05 * whole)

    def test_a_call_costs_a_branch_not_the_hundred_clocks_llvm_mca_assumes(self):
        # llvm-mca assumes 100 cycles for any `bl`/`blr` ("call instructions are not correctly modeled"): fib -O2 was
        # 64,037 modelled clocks for 7,653 instructions, 8.4 a instruction, every call block priced at 103.
        code = {0x100: "mov x21, x0", 0x104: "bl 0x200 <g>"}
        clocks_, instructions = clocks.modelled_clocks(code, [0x100, 0x104])
        self.assertEqual(instructions, 2)
        self.assertLess(clocks_, 10)

    def test_an_adr_to_an_address_no_symbol_covers_is_priced(self):
        # gcc's `adr x0, 0x200158` (a literal in the same section) made llvm-mca refuse the block: jpeg_fixed and 13 more
        # gcc/clang cells were dropped from the board for want of a price.
        self.assertIsNotNone(clocks.mca_cycles(["adr x0, 0x200158", "add x0, x0, #1"], 1))

    def test_an_immediate_in_hex_is_not_a_target(self):
        # A first fix rewrote `add w3, w0, #0x1` to `##16`: the block was refused again.
        self.assertIsNotNone(clocks.mca_cycles(["add w3, w0, #0x1", "adr x2, 0x221bd8 <bmp>"], 1))
        self.assertIsNotNone(clocks.mca_cycles(["mov x1, #-0x1"], 1))

    def test_a_store_that_bumps_its_base_does_not_wait_for_its_data(self):
        # llvm-mca makes `strh w6, [x8], #2` write x8 only after w6, which a load through x8 made: a copy loop priced 5.06
        # a trip against 1.10 for the same loop with a separate `add`, scroll -O1 +47% over clang for it alone.
        bumped = ["ldrh w6, [x8, #0xa0]", "subs x7, x7, #0x1", "strh w6, [x8], #0x2", "b.ne 0x100 <f>"]
        split = ["ldrh w6, [x8, #0xa0]", "subs x7, x7, #0x1", "strh w6, [x8]", "add x8, x8, #0x2", "b.ne 0x100 <f>"]
        self.assertAlmostEqual(clocks.mca_cycles(bumped, 100), clocks.mca_cycles(split, 100), delta=0.2)
        pre = ["ldrh w6, [x8, #0xa0]", "subs x7, x7, #0x1", "strh w6, [x8, #0x2]!", "b.ne 0x100 <f>"]
        self.assertLess(clocks.mca_cycles(pre, 100), 2.5)
        # A prologue's `stp x30, x19, [sp, #-16]!` and a downward copy's `strh w6, [x8], #-2` still parse.
        self.assertIsNotNone(clocks.mca_cycles(["stp x30, x19, [sp, #-0x10]!", "strh w6, [x8], #-0x2"], 1))


if __name__ == "__main__":
    unittest.main()
