// flags: -O0 | -O2 | -Os
// A 64-bit datum in a global (gcc.c-torture 20000314-2 `const uint64 bigconst = 1ULL << 34`): the front end spells it DGInteger64,
// which the data reader refused ("data item DGInteger64 17179869184 TY_UINT_8").
extern void report(long value);
typedef unsigned long long u64;
typedef long long s64;
const u64 big = 1ULL << 34;
u64 most = 0xFFFFFFFFFFFFFFFFULL;
s64 neg = -5000000000LL;
s64 table[3] = { 1LL << 40, -2, 77 };
int main(void)
{
    report((long)(big >> 30));
    report((long)(most >> 60));
    report((long)(neg / 1000000));
    report((long)(table[0] >> 38) + (long)table[1] + (long)table[2]);
    return 0;
}
