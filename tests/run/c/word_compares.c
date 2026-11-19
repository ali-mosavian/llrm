// flags: -O0 | -O2 | -Os
// A compare of two 32-bit words, every order, signed and unsigned, over the values that tell the halves apart:
// the borrow of the low words into the high words' subtract (a wrong carry flips one order at one pair).
extern void report(long value);

typedef __INT32_TYPE__ s32;
typedef __UINT32_TYPE__ u32;

static volatile u32 v[] = {0u, 1u, 2u, 0x7fffu, 0x8000u, 0xffffu, 0x10000u, 0x10001u,
                           0x7fffffffu, 0x80000000u, 0x80000001u, 0xffff0000u, 0xfffffffeu, 0xffffffffu};

static u32 orders(u32 a, u32 b)
{
    s32 x = (s32)a, y = (s32)b;
    return (a < b) + 2 * (a <= b) + 4 * (a > b) + 8 * (a >= b) + 16 * (x < y) + 32 * (x <= y) + 64 * (x > y) +
           128 * (x >= y);
}

int main(void)
{
    int i, j;
    for (i = 0; i < 14; ++i) {
        u32 h = 0;
        for (j = 0; j < 14; ++j)
            h = h * 257u + orders(v[i], v[j]);
        report((long)(s32)h);
    }
    return 0;
}
