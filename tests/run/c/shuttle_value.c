// flags: -O1 | -O2 | -Os
// A value copied into a register an operation then works in and read again after a call (gcc.c-torture shiftdi, adapted: the
// 32-bit `w` is passed to a call, then shifted again by the count it was masked with). The peephole's shuttle rewrite
// (`T = S; T = op(T); S = T` is `S = op(S); T = S`) dropped the definition of the copy a later block still read, and the
// compile failed: "peephole: value#31 is read but never defined or supplied by the caller".
extern void report(long value);

typedef unsigned long long uint64;

void g(uint64 x, int y, int z, uint64 *p)
{
    unsigned long w = (unsigned long)(((x >> y) & 0xffffffffULL) << (z & 0x1f));
    report((long)w);
    *p |= ((uint64)w & 0xffffffffULL) << z;
}

int main(void)
{
    uint64 a = 0, b = 0, c = 0;
    g(0xdeadbeef01234567ULL, 0, 0, &a);
    g(0x123456789abcdef0ULL, 8, 3, &b);
    g(0xffffffffffffffffULL, 33, 37, &c);
    report((long)a);
    report((long)(b >> 32));
    report((long)b);
    report((long)(c >> 32));
    report((long)c);
    return 0;
}
