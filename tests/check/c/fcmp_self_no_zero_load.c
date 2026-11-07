// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// `x != x` becomes `fcmp uno x, 0.0`; the pin's SimplifySetCC makes it `setuo x, x` so no 0.0 is loaded. The port kept the
// constant: fldz + fcomp where fst/fcom of x itself serves (sfm16_float_add m32-dos -O2 +6 B after the IR fold).
// CHECK-LABEL: _f proc
// CHECK-NOT: fldz
// CHECK: _f endp
int f(float x)
{
    return x != x;
}
