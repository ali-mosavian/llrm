// RUN: llrm-cc %s -m32 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// The constant 1.0 read three times was three `fld1` and popping adds; the
// pin loads it once and adds in place (nbody_single: +4.1% clocks).
float falloff(float x, float y)
{
    float d = x * x + y * y + 1.0f;
    return 1.0f / (d + 1.0f);
}
// CHECK-LABEL: falloff_ proc
// CHECK: fld1
// CHECK-NOT: fld1
