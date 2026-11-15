// flags: -O2
// bar: none (SDCC error 101: too many parameters)
// __builtin_clzll and ctzll of a 64-bit value stopped the x86_64 DAG at emit (`Unmapped("BSR64rr")`, `BSF64rr`): the m64 forms had no bsf or bsr.
extern void report(long value);

__attribute__((noinline)) int leading(unsigned long long x) { return __builtin_clzll(x); }
__attribute__((noinline)) int trailing(unsigned long long x) { return __builtin_ctzll(x); }

int main(void)
{
    volatile unsigned long long one = 1, top = 0x8000000000000000ull, mid = 0x0000100000000000ull;

    report(leading(one));
    report(leading(top));
    report(leading(mid));
    report(trailing(one));
    report(trailing(top));
    report(trailing(mid));
    return 0;
}
