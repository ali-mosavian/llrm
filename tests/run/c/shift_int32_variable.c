/* A 32-bit shift by every count 0..31, the count a variable: the library routines __ashlsi3, __lshrsi3 and
   __ashrsi3 on a 16-bit target. Three values, shl, lshr and ashr of each. */
extern void report(long value);
typedef __UINT32_TYPE__ u32;
typedef __INT32_TYPE__ i32;

__attribute__((noinline)) static u32 shl(u32 x, int n) { return x << n; }
__attribute__((noinline)) static u32 shr(u32 x, int n) { return x >> n; }
__attribute__((noinline)) static i32 sar(i32 x, int n) { return x >> n; }
/* a second caller of each routine, so no one of them is a single-use routine inlined into its caller */
__attribute__((noinline)) static u32 shl2(u32 x, int n) { return x << n; }
__attribute__((noinline)) static u32 shr2(u32 x, int n) { return x >> n; }
__attribute__((noinline)) static i32 sar2(i32 x, int n) { return x >> n; }

int main(void)
{
    static const u32 xs[3] = {0x01234567u, 0x80000001u, 0xFFFFFFFFu};
    volatile int n;
    int i, k;
    for (i = 0; i < 3; i++)
        for (k = 0; k < 32; k++) {
            n = k;
            report((long)(i32)(k & 1 ? shl2(xs[i], n) : shl(xs[i], n)));
            report((long)(i32)(k & 1 ? shr2(xs[i], n) : shr(xs[i], n)));
            report((long)(k & 1 ? sar2((i32)xs[i], n) : sar((i32)xs[i], n)));
        }
    return 0;
}
