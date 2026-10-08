extern void report(long value);

static unsigned short a[64], b[64], c[65];
long bench_x_bigadd(int n)
{
    int i, r; unsigned carry; long sum = 0;
    for (i = 0; i < 64; ++i) { a[i] = (unsigned short)(i * 997 + 13); b[i] = (unsigned short)(i * 331 + 7); }
    for (r = 0; r < n; ++r) {
        carry = 0;
        for (i = 0; i < 64; ++i) { unsigned t = (unsigned)a[i] + b[i] + carry; c[i] = (unsigned short)t; carry = t >> 16; }
        c[64] = (unsigned short)carry;
        for (i = 0; i < 64; ++i) a[i] = (unsigned short)(c[i] ^ r);
        sum += c[63];
    }
    return sum + c[64];
}

int main(void)
{
    report(bench_x_bigadd(200));
    return 0;
}
