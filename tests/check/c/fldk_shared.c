// RUN: llrm-cc %s -m32 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// The constant 0.01f read four times was four `fld` of the pool entry, each
// folded into its own `fmul`/`fadd [K]`; the pin loads it once, as the load
// has four uses (x_nbody_f -O2: +16 B, 21 folds against 1).
float kernel(float a, float b, float c)
{
    float d = a * a + 0.01f;
    return a * 0.01f + b * 0.01f + c * 0.01f + d * 0.01f;
}
// CHECK-LABEL: kernel_ proc
// CHECK: fld dword ptr $K
// CHECK-NOT: $K
