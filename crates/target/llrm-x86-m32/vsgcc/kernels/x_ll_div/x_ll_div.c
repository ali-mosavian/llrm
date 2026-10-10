extern void report(long value);

/* 64-bit divides by a variable: a dword divisor (the common case), a wider one, unsigned and signed. */
long bench_x_ll_div(int n)
{
    unsigned long long a = 0x123456789ABCDEF1ull, u = 0; long long s = 0; int i;
    for (i = 0; i < n; ++i) {
        unsigned long long d32, d64; long long sa, sd;
        a = a * 6364136223846793005ull + 1442695040888963407ull;
        d32 = (a >> 40) | 1; d64 = (a >> (11 + (i & 15))) | 1;
        u += a / d32 + a % d32;
        u += a / d64 + a % d64;
        sa = (long long)a; sd = (long long)(d64 ^ (a << 20));
        sd = sd ? sd : 3;
        s += sa / (long long)d32 + sa % (long long)d32;
        s += sa / sd + sa % sd;
    }
    return (long)((u ^ (unsigned long long)s) & 0x7FFFFFFF);
}

int main(void)
{
    report(bench_x_ll_div(200));
    return 0;
}
