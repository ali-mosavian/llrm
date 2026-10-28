// RUN: llrm-c %s -m16 -Os -S -o /dev/stdout
// matmul's fill nest at -Os in real mode: LSR's own model rates the rewrite of `(i + j) % 3` worse than the loop as written, which the
// 16-bit target keeps (`shouldDropLSRSolutionIfLessProfitable`). Rewritten, the loop carries a second counter and rebuilds the remainder
// with `div` then `imul` and `sub`: +13 B and +355 executed instructions on bench matmul.
// CHECK-LABEL: _fill@3 proc
// CHECK: div si
// CHECK-NOT: imul
// CHECK: _fill@3 endp
void fill(short a[8][8], short b[8][8], unsigned short seed)
{
    unsigned short i, j;
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) {
            a[i][j] = (short)(i * 3 + j + 1 + seed);
            b[i][j] = (short)(i == j ? 2 : (i + j) % 3);
        }
}
