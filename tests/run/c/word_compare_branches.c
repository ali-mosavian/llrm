// flags: -O0 | -O2 | -Os
// A branch and a select on the order of two 32-bit words, every order, signed and unsigned, over the values that tell
// the halves apart: the flags the borrow chain leaves are read by `bcc`/`bcs`, and a wrong carry flips one order at one pair.
extern void report(long value);

typedef __INT32_TYPE__ s32;
typedef __UINT32_TYPE__ u32;

static volatile u32 v[] = {0u, 1u, 2u, 0x7fffu, 0x8000u, 0xffffu, 0x10000u, 0x10001u,
                           0x7fffffffu, 0x80000000u, 0x80000001u, 0xffff0000u, 0xfffffffeu, 0xffffffffu};

static u32 branches(u32 a, u32 b)
{
    s32 x = (s32)a, y = (s32)b;
    u32 r = 0;
    if (a < b) r += 1;
    if (a <= b) r += 2;
    if (a > b) r += 4;
    if (a >= b) r += 8;
    if (x < y) r += 16;
    if (x <= y) r += 32;
    if (x > y) r += 64;
    if (x >= y) r += 128;
    return r;
}

static u32 selects(u32 a, u32 b)
{
    s32 x = (s32)a, y = (s32)b;
    return (a < b ? 3u : 5u) + (a >= b ? 7u : 11u) * 16u + (x < y ? 13u : 17u) * 256u + (x >= y ? 19u : 23u) * 4096u;
}

int main(void)
{
    int i, j;
    for (i = 0; i < 14; ++i) {
        u32 h = 0;
        for (j = 0; j < 14; ++j)
            h = h * 257u + branches(v[i], v[j]) + selects(v[i], v[j]) * 65599u;
        report((long)(s32)h);
    }
    return 0;
}
