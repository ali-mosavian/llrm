// RUN: llrm-cc %s -m64 -O1 -S -o /dev/stdout
// A split of a hoisted address left the copy dead and, behind it, the address itself: dead `mov rcx, offset X` before
// the loop, never read (overflow_builtins -O1 657 B against 636 B).
// CHECK-LABEL: main proc near
// CHECK-NOT: mov rcx, offset
// CHECK: align 16
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

int main(void)
{
    volatile short *s = sa;
    volatile unsigned short *us = ua;
    volatile long *l = la;
    volatile unsigned long *ul = ula;
    volatile i64 *q = qa;
    int i, j;
    for (i = 0; i < 3; i++)
        for (j = 0; j < 3; j++) {
            CASE(short, add, s[i], s[j]);
            CASE(short, sub, s[i], s[j]);
            CASE(unsigned short, add, us[i], us[j]);
            CASE(unsigned short, sub, us[i], us[j]);
            CASE(long, add, l[i], l[j]);
            CASE(long, sub, l[i], l[j]);
            CASE(unsigned long, add, ul[i], ul[j]);
            CASE(unsigned long, sub, ul[i], ul[j]);
            CASE(i64, add, q[i], q[j]);
        }
}

