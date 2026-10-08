// RUN: llrm-c %s -m32 -O2 -march=i486 -fno-inline-functions -S -o /dev/stdout
// bench/tile (7 instructions an element, clang 4) and bench/nbody (`inc bx; movzx esi,bx; cmp bx,4`) on -m32.
// A mask the counter's range makes a no-op is gone: one load, one add, the step, the test.
// CHECK-LABEL: masked_ proc
// CHECK: movsx {{e..}}, word ptr
// CHECK-NEXT: add
// CHECK-NEXT: add
// CHECK-NEXT: jne
// CHECK: masked_ endp
// A counter that starts where the outer loop left it is one the width of its index.
// CHECK-LABEL: from_ proc
// CHECK: fsub
// CHECK-NEXT: fld
// CHECK-NEXT: fmulp
// CHECK-NEXT: faddp
// CHECK-NEXT: add
// CHECK-NEXT: jne
// CHECK: from_ endp
static short t[64][64];
long masked(void)
{
    short x, y;
    long total = 0;
    for (y = 0; y < 25; ++y)
        for (x = 0; x < 40; ++x) total += t[(y + 61) & 63][(x + 7) & 63];
    return total;
}
double from(double *x, double *v, unsigned short i)
{
    unsigned short j;
    double s = 0;
    for (j = i + 1; j < 4; ++j) s += (x[j] - x[i]) * v[j];
    return s;
}
