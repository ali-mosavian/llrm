"""`uv run --project tools python -m unittest discover -s crates/target/llrm-x86-m16/bench -p "test_over_aligned.py"`

An explicit `aligned(N)` local on the m16 target, observed in the emulator: its address `& (N-1)` read across a call (in one function the
optimizer folds it to 0 from the attribute). Real mode keeps the stack two-byte aligned and the frame does not realign, so the address
is off its alignment at every level (56, 54, 50 at -O0; 60 at -O2/-Os, frame depth moving it). Known wrong, debt KNOWN-WRONG-ALIGN-M16
(docs/working/ci.md): each case asserts the address IS misaligned and fails the day it is aligned, so the debt row and these cases go
together. A build or run error is an error, never a pass. The control (aligned(2)) proves the harness reads the residue.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
REPO = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO / "tools" / "dosbatch"))
sys.path.insert(0, str(REPO / "tools" / "bench"))

import volatile_order  # noqa: E402

PROGRAM = r"""
extern void report(long value);
__attribute__((noinline, optnone)) long low(char *p) { return (long)((unsigned)(unsigned long)p & @MASK@); }
__attribute__((noinline)) long f(void) { char a[@SIZE@] __attribute__((aligned(@ALIGN@))); volatile char *q = a; q[0] = 1; return low((char *)q); }
__attribute__((noinline)) long g(int n) { volatile char pad[@PAD@]; pad[0] = n; return f() + pad[0] - n; }
int main(void) { report(g(1)); return 0; }
"""


def residue(align: int, pad: int, level: str) -> int:
    source = PROGRAM
    for token, value in (("@MASK@", align - 1), ("@SIZE@", align * 2), ("@ALIGN@", align), ("@PAD@", pad)):
        source = source.replace(token, str(value))
    return int(volatile_order.accesses(source, [], level)[1])


class OverAligned(unittest.TestCase):
    def test_control_a_word_aligned_local_reads_zero(self):
        for level in ("-O0", "-O2", "-Os"):
            self.assertEqual(residue(2, 1, level), 0, level)


def case(align, pad, level):
    def run(self):
        if residue(align, pad, level) == 0:
            self.fail(f"aligned({align}) is now honoured at {level}: remove debt KNOWN-WRONG-ALIGN-M16 and this case")

    return run


for wanted in (16, 32):
    for padding in (1, 5):
        for opt in ("-O0", "-O2", "-Os"):
            setattr(OverAligned, f"test_known_wrong_aligned_{wanted}_pad_{padding}_{opt.strip('-')}", case(wanted, padding, opt))

if __name__ == "__main__":
    unittest.main()
