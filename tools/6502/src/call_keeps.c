// flags: -O2 -fno-inline-functions | -O2 -fno-inline-functions -m32
// A 32-bit value held across a call keeps its upper half whatever the callee does with its registers: the callee
// here needs every register as a 32-bit value, saves what it uses and gives it back whole.
extern void report(long value);

static unsigned long churn(unsigned long a, unsigned long b, unsigned long c, unsigned long d)
{
    unsigned long p = a * 3 + b, q = b * 5 + c, r = c * 7 + d, s = d * 11 + a, t = a ^ d, u = b ^ c;

    p = (p << 3) ^ (q >> 2) ^ t;
    q = (q << 5) ^ (r >> 3) ^ u;
    r = (r << 7) ^ (s >> 5) ^ p;
    s = (s << 9) ^ (p >> 7) ^ q;
    return p + q + r + s + t + u;
}

static unsigned long hold(unsigned long n)
{
    unsigned long w = 0x12345678UL + n, x = 0x9ABCDEF0UL ^ n, y = 0xFEDCBA98UL - n, z = 0x80000001UL + n * 3;
    unsigned long first = churn(w, x, y, z);
    unsigned long second = churn(x, y, z, w + first);

    return (w ^ x) + (y ^ z) + first + second;
}

int main(void)
{
    report((long)hold(1));
    report((long)hold(77));
    report((long)hold(0xFFFFUL));
    return 0;
}
