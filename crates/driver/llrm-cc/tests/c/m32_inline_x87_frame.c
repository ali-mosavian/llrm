/* QB's pfpu_rint32: x87 inline code whose memory operand is a local. */
static inline int rint32(float x) {
    int v;
    __asm__("fistpl %0" : "=m"(v) : "t"(x) : "st");
    return v;
}

/* Many values live across the loop: a frame register held back costs copies. */
int f(const float *a, int n, int *out, int k0, int k1, int k2, int k3) {
    int s0 = 0, s1 = 0, s2 = 0, s3 = 0;
    for (int i = 0; i < n; i++) {
        s0 += rint32(a[i]) + k0;
        s1 += s0 ^ k1;
        s2 += s1 + k2;
        s3 += s2 ^ k3;
        out[i] = s3;
    }
    return s0 + s1 + s2 + s3;
}
