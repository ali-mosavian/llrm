/* sfm16_dbg: `if (exponent != 255 && bits(a + b) != want)`; the sum is made
 * before the test in the source, and the test skips it 16 times in 400. */
extern const unsigned V[][11];
typedef unsigned u32;
static float bits(u32 u) { union { u32 u; float f; } x; x.u = u; return x.f; }
static u32 ubits(float f) { union { u32 u; float f; } x; x.f = f; return x.u; }

long mismatches(int n)
{
    long bad = 0, first = -1;
    for (int i = 0; i < n; ++i) {
        float c = bits(V[i][0]) + bits(V[i][1]);
        u32 u = ubits(c);
        if (((V[i][2] >> 23) & 0xFF) != 0xFF && u != V[i][2]) {
            if (!bad) first = i;
            ++bad;
        }
    }
    return bad * 1000 + first;
}
