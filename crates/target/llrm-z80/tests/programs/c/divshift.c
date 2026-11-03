extern void report(long value);
typedef long long s64;
typedef unsigned long long u64;
__attribute__((noinline)) u64 udiv(u64 a, u64 b) { return a / b; }
__attribute__((noinline)) u64 umod(u64 a, u64 b) { return a % b; }
__attribute__((noinline)) s64 sdiv(s64 a, s64 b) { return a / b; }
__attribute__((noinline)) s64 smod(s64 a, s64 b) { return a % b; }
__attribute__((noinline)) u64 shl(u64 a, int n) { return a << n; }
__attribute__((noinline)) u64 shr(u64 a, int n) { return a >> n; }
__attribute__((noinline)) s64 sar(s64 a, int n) { return a >> n; }
static void two(u64 v) { report((long)v); report((long)(v >> 32)); }
int main(void)
{
    volatile u64 a = 0x123456789ABCDEF0ULL, b = 1000003ULL;
    volatile s64 c = -0x123456789ABCDEFLL, d = 7919;
    two(udiv(a, b));
    two(umod(a, b));
    two((u64)sdiv(c, d));
    two((u64)smod(c, d));
    two((u64)sdiv(c, -d));
    two((u64)smod(-c, -d));
    two(shl(a, 13));
    two(shr(a, 37));
    two((u64)sar(c, 21));
    two(shl(a, 3));
    two(shr(a, 0));
    two(udiv(a, 0x100000001ULL));
    return 0;
}
