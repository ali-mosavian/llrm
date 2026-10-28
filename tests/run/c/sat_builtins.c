// flags: -O1 | -O2 | -Os
// __builtin_elementwise_{add,sub}_sat of 32 and 64 bit signed and unsigned integers (a narrower type promotes to int
// first): the shape clang gives llvm.{u,s}{add,sub}.sat (the intrinsic was a refusal at every width).
extern void report(long value);

typedef long long i64;
typedef unsigned long long u64;

#define CASE(T, OP, A, B) \
    do { T r = __builtin_elementwise_##OP##_sat(A, B); report((long)r); report((long)(r / 256)); } while (0)

long la[3] = {2000000000L, -2000000000L, 70000L};
unsigned long ula[3] = {4000000000UL, 5UL, 70000UL};
i64 qa[3] = {0x7000000000000000LL, -0x7000000000000000LL, 3000000000LL};
u64 uqa[3] = {0xF000000000000000ULL, 5ULL, 4000000000ULL};

int main(void)
{
    volatile long *l = la;
    volatile unsigned long *ul = ula;
    volatile i64 *q = qa;
    volatile u64 *uq = uqa;
    int i, j;
    for (i = 0; i < 3; i++)
        for (j = 0; j < 3; j++) {
            CASE(long, add, l[i], l[j]);
            CASE(long, sub, l[i], l[j]);
            CASE(unsigned long, add, ul[i], ul[j]);
            CASE(unsigned long, sub, ul[i], ul[j]);
            CASE(i64, add, q[i], q[j]);
            CASE(i64, sub, q[i], q[j]);
            CASE(u64, add, uq[i], uq[j]);
            CASE(u64, sub, uq[i], uq[j]);
        }
    return 0;
}
