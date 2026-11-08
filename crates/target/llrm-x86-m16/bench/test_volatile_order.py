"""`uv run --project tools python -m unittest discover -s crates/target/llrm-x86-m16/bench -p "test_volatile_order.py"`

The volatile loads of a call's arguments happen in the order the source gives them, at every level, in the emulator: observed reads
of the globals against the source order. The control calls with another order and must observe that one, so a log that never changes
cannot pass.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
REPO = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO / "tools" / "dosbatch"))
sys.path.insert(0, str(REPO / "tools" / "bench"))

import volatile_order  # noqa: E402

LEVELS = ["-O0", "-O1", "-O2", "-O3", "-Os"]

INTS = """
extern void report(long value);
volatile int g1 = 1, g2 = 2, g3 = 3;
__attribute__((noinline)) int f(int a, int b, int c) { return a * 100 + b * 10 + c; }
__attribute__((noinline)) int run(void) { return f({args}); }
int main(void) { report(run()); return 0; }
"""

# 32-bit volatiles are two half-word reads or one dword read: the order of the variables is what holds.
LONGS = """
extern void report(long value);
volatile long g1 = 100000L, g2 = 20000L, g3 = 3000L;
__attribute__((noinline)) long f(long a, long b, long c) { return a + b / 10 + c / 100; }
__attribute__((noinline)) long run(void) { return f(g1, g2, g3); }
int main(void) { report(run()); return 0; }
"""

# five arguments: the first ones travel in registers, the rest on the stack
MIXED = """
extern void report(long value);
volatile int g1 = 1, g2 = 2, g3 = 3, g4 = 4, g5 = 5;
__attribute__((noinline)) int f(int a, int b, int c, int d, int e) { return a * 10000 + b * 1000 + c * 100 + d * 10 + e; }
__attribute__((noinline)) int run(void) { return f(g1, g2, g3, g4, g5); }
int main(void) { report(run()); return 0; }
"""


class VolatileOrder(unittest.TestCase):
    def observe(self, source, names, level):
        return volatile_order.accesses(source, names, level)

    def test_int_arguments_are_read_in_source_order(self):
        for level in LEVELS:
            with self.subTest(level=level):
                order, output = self.observe(INTS.replace("{args}", "g1, g2, g3"), ["g1", "g2", "g3"], level)
                self.assertEqual((order, output), (["g1", "g2", "g3"], b"123"))

    def test_the_control_with_another_order_observes_that_order(self):
        for level in LEVELS:
            with self.subTest(level=level):
                order, output = self.observe(INTS.replace("{args}", "g3, g1, g2"), ["g1", "g2", "g3"], level)
                self.assertEqual((order, output), (["g3", "g1", "g2"], b"312"))

    def test_32_bit_arguments_are_read_in_source_order(self):
        for level in LEVELS:
            with self.subTest(level=level):
                order, _ = self.observe(LONGS, ["g1", "g2", "g3"], level)
                self.assertEqual(order, ["g1", "g2", "g3"])

    def test_registers_and_stack_arguments_are_read_in_source_order(self):
        for level in LEVELS:
            with self.subTest(level=level):
                order, output = self.observe(MIXED, ["g1", "g2", "g3", "g4", "g5"], level)
                self.assertEqual((order, output), (["g1", "g2", "g3", "g4", "g5"], b"12345"))


if __name__ == "__main__":
    unittest.main()
