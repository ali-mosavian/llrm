// flags: -O1 | -O2 | -Os
// __builtin_{add,sub,mul}_overflow of short, long and unsigned: each result and its flag, the shape clang gives
// llvm.{u,s}{add,sub,mul}.with.overflow (a flag the legalizer promoted, or an i64 it expanded, was a refusal).
extern void report(long value);

typedef long long i64;
typedef unsigned long long u64;

#define CASE(T, OP, A, B) \
    do { T r; int o = __builtin_##OP##_overflow(A, B, &r); report(o); report((long)r); } while (0)

short sa[3] = {32000, -32000, 7};
unsigned short ua[3] = {60000, 5, 300};
long la[3] = {2000000000L, -2000000000L, 70000L};
unsigned long ula[3] = {4000000000UL, 5UL, 70000UL};
i64 qa[3] = {0x7000000000000000LL, -0x7000000000000000LL, 3000000000LL};
u64 uqa[3] = {0xF000000000000000ULL, 5ULL, 4000000000ULL};

int main(void)
{
    volatile short *s = sa;
    volatile unsigned short *us = ua;
    volatile long *l = la;
    volatile unsigned long *ul = ula;
    volatile i64 *q = qa;
    volatile u64 *uq = uqa;
    int i, j;
    for (i = 0; i < 3; i++)
        for (j = 0; j < 3; j++) {
            CASE(short, add, s[i], s[j]);
            CASE(short, sub, s[i], s[j]);
            CASE(short, mul, s[i], s[j]);
            CASE(unsigned short, add, us[i], us[j]);
            CASE(unsigned short, sub, us[i], us[j]);
            CASE(unsigned short, mul, us[i], us[j]);
            CASE(long, add, l[i], l[j]);
            CASE(long, sub, l[i], l[j]);
            CASE(long, mul, l[i], l[j]);
            CASE(unsigned long, add, ul[i], ul[j]);
            CASE(unsigned long, sub, ul[i], ul[j]);
            CASE(unsigned long, mul, ul[i], ul[j]);
            CASE(i64, add, q[i], q[j]);
            CASE(i64, sub, q[i], q[j]);
            CASE(i64, mul, q[i], q[j]);
            CASE(u64, add, uq[i], uq[j]);
            CASE(u64, sub, uq[i], uq[j]);
            CASE(u64, mul, uq[i], uq[j]);
        }
    return 0;
}
