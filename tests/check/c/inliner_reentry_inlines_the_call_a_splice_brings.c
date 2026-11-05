// RUN: llrm-cc %s -m32 -Os -march=i486 -w -S -o /dev/stdout
// `g` inlined into `main` brings a call of `f`, now f's last call: the pin prices it (cost -14930, threshold 50) and inlines it.
// Our worklist compared call counts against ones taken before the round's own `f` splices and never priced it: main kept
// `call f_` (1151 B of asm text against 235).
void report(long);
static long mix(long a, long b, long c, long d, long e, long f, long g)
{
    return a + 2 * b + 3 * c + 4 * d + 5 * e + 6 * f + 7 * g;
}
static long f(long n, long m, long k)
{
    long x, y, z;
    if (n == 0) return m;
    x = mix(n, m, k, 4, 5, 6, 7);
    y = mix(x, n, m, k, 1, 2, 3);
    z = mix(y, x, n, m, k, 0, 1);
    return x + y + z + n * m + k;
}
static long g(long n, long m, long k)
{
    if (n > 3) return n;
    return f(n, m, k) + g(n + 1, m + 1, k);
}
int main(void)
{
    report(f(0, 9, 3));
    report(f(2, 5, 1));
    report(g(0, 2, 7));
    return 0;
}

// CHECK-LABEL: main_ proc
// CHECK-NOT: call f_
// CHECK: main_ endp
