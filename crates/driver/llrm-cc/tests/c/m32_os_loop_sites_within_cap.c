/* Six calls of a hinted callee sit in `main`'s loop, whose body runs a long
 * `heavy` call: keeping the calls adds well under 2% of the clocks and saves
 * the five extra copies. */
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

__attribute__((noinline)) u32 heavy(u32 i)
{
    u32 s = i;
    for (u32 a = 0; a < 64; a++)
        for (u32 b = 0; b < 64; b++)
            for (u32 c = 0; c < 64; c++)
                s = s * 31u + a + b * c;
    return s;
}

int main(void)
{
    int sum = 0;
    for (u32 i = 1; i < 1000; i++)
        sum += (int)heavy(i) + lead_zeros(i) + lead_zeros(i + 1) + lead_zeros(i * 3) + lead_zeros(i ^ 5)
             + lead_zeros(i | 9) + lead_zeros(i - 7);
    return sum;
}
