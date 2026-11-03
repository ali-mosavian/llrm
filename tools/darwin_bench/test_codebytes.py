"""kernel() must not count the harness. novec.py once reported code_bytes() whole: every kernel was 36-60 B too big
(crc O2 pin 192 vs 156), which moved the small kernels across the 10% line. Run: python3 test_codebytes.py"""
import os, subprocess, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from codebytes import code_bytes, kernel
SRC = """__attribute__((noinline)) int bench_k(int x) { return x * 3 + 1; }
void report(int v) { volatile int s = v; (void)s; }
int orig_main(void) { report(bench_k(2)); report(bench_k(3)); return 0; }
"""
with tempfile.TemporaryDirectory() as d:
    open(f"{d}/k.c", "w").write(SRC)
    subprocess.check_call(["clang", "-O2", "-c", f"{d}/k.c", "-o", f"{d}/k.o"])
    b, n = kernel(f"{d}/k.o")
    assert (b, n) == (12, 3), f"kernel() = {b} B / {n} insns, want 12 B / 3 (add w0,w0,w0,lsl #1; add w0,w0,#1; ret)"
    assert code_bytes(f"{d}/k.o") > b, "the whole object must be bigger: it holds orig_main and report"
print("PASS")
