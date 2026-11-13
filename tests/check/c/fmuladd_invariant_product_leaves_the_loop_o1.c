// RUN: llrm-cc %s -m32 -mabi=sysv -O1 -march=i486 -S -o /dev/stdout
// x_ld_to_ulong m32 -O1: clang contracts `x[i] + r * 2^26` into `llvm.fmuladd`, which LICM keeps whole, so the
// product `r * 2^26` was made on every trip (`fld st(0); fmul` in the loop, 19206 clk). The 486 has no FMA: the
// product is split and leaves the loop (14970 clk, +2 B for the stack slot held across it).
extern long double x[64];

long bench(int n)
{
    unsigned long t = 0;
    for (int r = 0; r < 4; r++)
        for (int i = 0; i < n; i++)
            t = t * 5 + ((unsigned long)(x[i] + r * 67108864.0L) >> 24);
    return (long)t;
}

// CHECK-LABEL: _bench proc
// CHECK: fmul dword ptr
// CHECK: {{L[0-9_]+}}:
// CHECK: fadd st(0), st(1)
// CHECK-NOT: fmul
// CHECK: _bench endp
