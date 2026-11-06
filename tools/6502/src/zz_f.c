// flags: -O0 | -O2
extern void report(long value);
typedef unsigned long long u64;
int main(void)
{
    double d = (double) 18446744073709551615ULL;
    u64 big = 0xFFFFFFFFFFFFFFFFULL;
    double e = (double)big;
    report(d < 1.84467440737095e+19);
    report(d > 1.84467440737096e+19);
    report(16777217L != (float)16777217e0);
    report(e == d);
    return 0;
}
