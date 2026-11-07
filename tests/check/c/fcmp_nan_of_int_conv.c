// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// `(float)v != (float)v` ran fcom/fnstsw/sahf: an int converted to float is never a NaN, so `une x,x` is `uno x,0.0`
// and the pin folds it to false (sfm16_float_i2f m32 -O2: 18026 clk vs clang 11615). The port had no visitFCmpInst head.
// CHECK-LABEL: _f proc
// CHECK-NOT: fcom
// CHECK-NOT: fnstsw
// CHECK: xor eax, eax
// CHECK: _f endp
int f(long v)
{
    float x = (float)v;
    return x != x;
}
