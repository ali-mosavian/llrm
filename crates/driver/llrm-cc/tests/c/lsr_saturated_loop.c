typedef unsigned u32;
extern const u32 V[100][11];
static inline u32 shr_sticky(u32 m, int n) { if (n >= 32) return m != 0; return (m >> n) | ((m << (32 - n)) != 0); }
static u32 add_bits(u32 a, u32 b)
{
    if ((a & 0x7fffffffu) < (b & 0x7fffffffu)) { u32 t = a; a = b; b = t; }
    if ((a << 1) >= 0xff000000u) return a;
    if ((b << 1) == 0) return a;
    int ea = (a >> 23) & 255, eb = (b >> 23) & 255;
    u32 ma = ((a & 0x7fffff) | (ea ? 0x800000 : 0)) << 3, mb = ((b & 0x7fffff) | (eb ? 0x800000 : 0)) << 3;
    if (!ea) ea = 1;
    if (!eb) eb = 1;
    if (ea - eb > 25) return a;
    mb = shr_sticky(mb, ea - eb);
    u32 m;
    if ((a ^ b) >> 31) {
        m = ma - mb;
        if (!m) return 0;
        int s = __builtin_clz(m) - 5;
        if (s > ea - 1) s = ea - 1;
        m <<= s;
        ea -= s;
    } else {
        m = ma + mb;
        if (m & 0x8000000u) { m = (m >> 1) | (m & 1); ea++; }
    }
    if (ea >= 255) return (a & 0x80000000u) | 0x7f800000u;
    if (ea <= 0) { m = shr_sticky(m, 1 - ea); ea = 1; }
    m = (m + 3 + ((m >> 3) & 1)) >> 3;
    return (a & 0x80000000u) + ((u32)(ea - 1) << 23) + m;
}
u32 f(long n)
{
    u32 acc = 0;
    long i;
    for (i = 0; i < n; ++i) acc += add_bits(V[i % 100][0], V[i % 100][1] ^ 0x80000000u);
    return acc;
}
