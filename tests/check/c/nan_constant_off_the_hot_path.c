// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O1 -S -o /dev/stdout
// The NaN of `f != f ? NaN : bits` was made before the `jp` that picks it, so every sum paid for the constant of the rare unordered one (sfm16_float_add
// m32 -O1: 5599 instructions, 14812 clk, gcc 14427). A select's diamond states no odds; the target states the `jp` edge's (DEVIATE UNORDERED-ODDS), so the
// constant is made after the branch.
// CHECK-LABEL: _canon_bits proc
// CHECK-NOT: 2143289344
// CHECK: jp
// CHECK: 2143289344
// CHECK: _canon_bits endp
static unsigned canon(float f)
{
    union { unsigned u; float f; } x;
    if (f != f)
        return 0x7FC00000u;
    x.f = f;
    return x.u;
}
unsigned canon_bits(float a, float b)
{
    return canon(a + b);
}
