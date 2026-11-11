// RUN: llrm-cc %s -m64 -fno-pic -O3 -S -o /dev/stdout
// The byte price of a full unroll (UP1) vetoed this 16-trip loop in the callee, before the inliner ran, so the small rolled `scale` was inlined at every site (jpeg_fixed -O2 +530 B). The pin unrolls the callee first (CGSCC order): `scale` is unrolled and still called.
// CHECK-LABEL: run proc
// CHECK: call scale
// CHECK: run endp
extern int tab[16];
static void scale(int *p, int k)
{
    int i;

    for (i = 0; i < 16; ++i) p[i] = (p[i] * k + tab[i]) >> 3;
}
void run(int *a, int *b, int *c, int *d, int *e, int *f)
{
    scale(a, 3); scale(b, 5); scale(c, 7); scale(d, 9); scale(e, 11); scale(f, 13);
}
