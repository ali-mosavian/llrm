// RUN-PENDING: llrm-cc %s -m32 -O2 -march=i486 -fno-inline-functions -S -o /dev/stdout
// bench/tile (7 instructions an element, clang 4) and bench/nbody (`inc bx; movzx esi,bx; cmp bx,4`) on -m32.
// The inner loop of 40 is unrolled in full, as the pin does (clang -m32 -O2: loads at t+14 .. t+92); the outer counter is the one step by one and the test, the mask on the row stays.
// CHECK-LABEL: masked_ proc
// CHECK: and {{e..}}, 63
// CHECK-NEXT: shl {{e..}}, 7
// CHECK: movsx {{e..}}, word ptr _t+92[{{e..}}]
// CHECK: inc
// CHECK-NEXT: cmp
// CHECK-NEXT: jne
// CHECK: masked_ endp
// A counter that starts where the outer loop left it is one the width of its index.
// CHECK-LABEL: from_ proc
// CHECK: fsub
// CHECK-NEXT: fmul
// CHECK-NEXT: faddp
// CHECK-NEXT: inc
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
