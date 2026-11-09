// RUN: llrm-cc %s -m32 -mabi=sysv -O1 -march=i486 --isel=dag -S -o /dev/stdout
// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 --isel=dag -S -o /dev/stdout
// CS1: at i486 a select is a branch around a copy whatever is done, so the
// conversion moves into the arm that needs it: the constant path returns
// before `fistp` (x_ldcmp -O1 without it: +21.6% clocks).
// CHECK-LABEL: _clamp proc
// CHECK: mov eax, 3
// CHECK: ret
// CHECK: fistp
// CHECK: _clamp endp
int clamp(float x, int c)
{
    float s = c ? 3.0f : x;
    return (int)s;
}
