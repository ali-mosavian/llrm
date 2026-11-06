// flags: -O1 | -O2 | -Os
// Fixed-width `unsigned long` so m16 (16-bit `unsigned`) runs the same program. `((unsigned long long)a << 27) / b` was emitted with a 32-bit `shl eax, 27` before `div rcx`: the narrowed shift lost the top 27 bits (softfloat's divide, wrong quotients). Sums of 200000 quotients.
extern void report(long value);

__attribute__((noinline)) unsigned long f(unsigned long a, unsigned long b)
{
    unsigned long long n = (unsigned long long)a << 27;
    return (unsigned long)(n / b);
}

int main(void)
{
    unsigned long long sum = 0, x = 88172645463325252ULL;
    unsigned long i;

    for (i = 0; i < 200000; ++i) {
        unsigned long a, b;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        a = (unsigned long)x;
        b = (unsigned long)(x >> 32) | 1;
        sum += f(a, b);
    }
    report((long)(sum & 0x7fffffff));
    report((long)(f(0xfffffffful, 3) & 0x7fffffff));
    return 0;
}
