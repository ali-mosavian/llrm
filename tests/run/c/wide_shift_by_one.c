// flags: -O0 | -O2 | -Os
// A 32- and 64-bit value shifted by one bit, left, right and arithmetic, over values that put the bit that crosses
// each word (and each byte) boundary at 0 and 1: a lost or doubled carry between the words changes one result.
extern void report(long value);

typedef __INT32_TYPE__ s32;
typedef __UINT32_TYPE__ u32;
typedef __UINT64_TYPE__ u64;
typedef __INT64_TYPE__ s64;

static volatile u32 v[] = {0u, 1u, 0x7fffu, 0x8000u, 0x8001u, 0xffffu, 0x10000u, 0x7fffffffu, 0x80000000u, 0x80000001u, 0xffff0000u,
                           0xffffffffu, 0x12345678u, 0x9abcdef0u};

static u32 fold32(u32 h, u32 x)
{
    return h * 257u + x;
}

int main(void)
{
    int i, j;
    for (i = 0; i < 14; ++i) {
        u32 a = v[i], h = 0;
        h = fold32(h, a << 1);
        h = fold32(h, a >> 1);
        h = fold32(h, (u32)((s32)a >> 1));
        h = fold32(h, (a << 1) | (a >> 31));
        for (j = 0; j < 14; ++j) {
            u64 w = ((u64)a << 32) | v[j];
            s64 s = (s64)w;
            u64 l = w << 1, r = w >> 1;
            h = fold32(h, (u32)l);
            h = fold32(h, (u32)(l >> 32));
            h = fold32(h, (u32)r);
            h = fold32(h, (u32)(r >> 32));
            h = fold32(h, (u32)(s >> 1));
            h = fold32(h, (u32)((s >> 1) >> 32));
            h = fold32(h, (u32)((w << 1) | (w >> 63)));
        }
        report((long)(s32)h);
    }
    return 0;
}
