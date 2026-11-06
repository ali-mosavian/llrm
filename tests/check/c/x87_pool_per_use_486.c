// RUN: llrm-cc %s -m32 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// At i486 `fmul m` is 14 clocks and `fmul st` 16, so the same constant read
// five times stays a memory operand at each reader (x_nbody_f -O2: sharing
// it was +2.2% clocks, nbody_single +0.6%): DEVIATE X87-POOL-SHARE.
float kernel(float a, float b, float c, float d)
{
    float e = a * a + 0.01f;
    return a * 0.01f + b * 0.01f + c * 0.01f + d * 0.01f + e * 0.01f;
}
// CHECK-LABEL: kernel_ proc
// CHECK: $K
// CHECK: $K
// CHECK: $K
// CHECK: $K
