/* Two hinted callees. `lead_zeros` has six cold calls (`main` reaches one);
 * `mix` has six calls in `main`'s own loop, where keeping them costs clocks
 * past the cap. The loop bundle put back must not put the cold one back. */
typedef unsigned int u32;

static inline int lead_zeros(u32 x)
{
    int n = 0;
    if (!(x & 0xFFFF0000u)) { n += 16; x <<= 16; }
    if (!(x & 0xFF000000u)) { n += 8; x <<= 8; }
    if (!(x & 0xF0000000u)) { n += 4; x <<= 4; }
    if (!(x & 0xC0000000u)) { n += 2; x <<= 2; }
    if (!(x & 0x80000000u)) { n += 1; }
    return n;
}

static inline int mix(u32 x)
{
    x ^= x >> 15;
    x *= 0x2c1b3c6du;
    x ^= x >> 12;
    x *= 0x297a2d39u;
    x ^= x >> 15;
    return (int)(x & 0x7fffffff);
}

int norm_a(u32 m) { return lead_zeros(m) - 8; }
int norm_b(u32 m) { return lead_zeros(m) - 5; }
int norm_c(u32 m, u32 n) { return lead_zeros(m) + lead_zeros(n); }
int norm_d(u32 m) { return 31 - lead_zeros(m); }
int norm_e(u32 m, u32 n) { return lead_zeros(m & n) * 3; }

int main(void)
{
    int sum = norm_a(7);
    for (u32 i = 1; i < 1000; i++)
        sum += mix(i) + mix(i + 1) + mix(i * 3) + mix(i ^ 5) + mix(i | 9) + mix(i - 7);
    return sum;
}
