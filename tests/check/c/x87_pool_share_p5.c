// RUN: llrm-cc %s -m32 -O2 -march=pentium --isel=dag -S -o /dev/stdout
// At P5 `fmul m` and `fmul st` are both 3 clocks, so a pool constant read
// five times is one load and register operands (the pin's rule), 2 bytes a
// reader, not five 6-byte memory operands.
float kernel(float a, float b, float c, float d)
{
    float e = a * a + 0.01f;
    return a * 0.01f + b * 0.01f + c * 0.01f + d * 0.01f + e * 0.01f;
}
// CHECK-LABEL: kernel_ proc
// CHECK: fld dword ptr $K
// CHECK-NOT: $K
