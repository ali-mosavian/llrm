// RUN: llrm-cc %s -m32 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// RUN: llrm-cc %s -m16 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// `3.7*v` and `-3.7*v` were two multiplies (x_jpeg_single -O3: 110 where
// gcc has 92); the second is the first's negation, so one multiply serves
// both, the sign going into an fadd/fsub (exact in IEEE).
float f(float a, float b, float v)
{
    return (a + 3.7f * v) + (b + -3.7f * v);
}
// CHECK-LABEL: {{_?f(_|@3)}} proc
// CHECK: fmul
// CHECK-NOT: fmul
