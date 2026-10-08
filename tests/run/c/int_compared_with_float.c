// flags: -O0 | -O2 | -Os
// An integer compared with a float is converted to the float's type first, which rounds it (C11 6.3.1.8, 6.3.1.4p2): 16777217L is
// 16777216.0f, equal to the float (gcc.c-torture 920710-1). `ficomp` read the integer exactly and found them different; against a
// double a 32-bit integer is exact and the compare is exact, as is a 64-bit one only to 53 bits.
extern void report(long value);

typedef long long s64;

volatile long big = 16777217L;
volatile unsigned long wide = 4294967295UL;
volatile s64 large = 9007199254740993LL;
volatile float single = 16777216.0f;
volatile float single_wide = 4294967296.0f;
volatile double twin = 16777216.0;
volatile double twin_huge = 9007199254740992.0;

int main(void)
{
    report(big != single);
    report(big == single);
    report(big != twin);
    report(wide == single_wide);
    report(large == twin_huge);
    report(large != twin);
    report(single < big);
    return 0;
}
