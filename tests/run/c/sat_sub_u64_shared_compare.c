// flags: -O1 | -O2 | -Os
// bar: none (SDCC error 101: too many parameters)
// A 64-bit saturating subtract: the low words' borrow compare is shared with the high words' order, so the carry
// pass dropped the compare the order chain continued (the result came out 0 for 5 below 0xF000000000000000).
extern void report(long value);
typedef unsigned long long u64;
static const u64 v[] = {0ULL, 5ULL, 4000000000ULL, 0xF000000000000000ULL};
int main(void)
{
    int i, j;
    for (i = 0; i < 4; ++i)
        for (j = 0; j < 4; ++j) {
            u64 r = __builtin_elementwise_sub_sat(v[i], v[j]);
            report((long)r);
        }
    return 0;
}
