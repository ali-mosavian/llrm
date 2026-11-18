// RUN: llrm-cc %s -m32 -Os -march=i486 --isel=dag -S -o /dev/stdout
// recfloat at -Os m32: trying the header's value order moved the constants' loads onto the loop's entry edge (`fld; fldz; fxch`), block placement then
// rotated the loop behind a guard and the function grew 6 bytes. Where bytes are strict the order is the first exit's.
// CHECK-LABEL: bench_recfloat_ proc
// CHECK: fldz
// CHECK-NEXT: fld dword ptr
// CHECK-NOT: fxch
// CHECK-NOT: je
// CHECK: ret
static double decay(double x, double acc, int n)
{
    if (n == 0) return acc;
    return decay(x * 0.5 + 1.0, acc + x, n - 1);
}

long bench_recfloat(int n)
{
    return (long)(decay(100.0, 0.0, n) * 1000.0 + 0.5);
}
