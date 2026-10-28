// flags: -O2 -march=i486 -fno-inline=sum5 -fno-inline=big_sum -fno-inline=make_big -fno-inline=vsum -fno-inline=half -fno-inline=twice | -O2 -march=i486 -mabi=regparm3 -fno-inline=sum5 -fno-inline=big_sum -fno-inline=make_big -fno-inline=vsum -fno-inline=half -fno-inline=twice | -O2 -march=i486 -m32 -fno-inline=sum5 -fno-inline=big_sum -fno-inline=make_big -fno-inline=vsum -fno-inline=half -fno-inline=twice
/* Calls in tail position that must stay calls, or jump with the callee's pops: a callee that pops its stack
   arguments, a call with more stack arguments than the caller has, a structure passed and returned by value, a
   variadic call, a float result and a call through a pointer. Each result is checked against a host compiler's. */
typedef __builtin_va_list va_list;
#define va_start(ap, last) __builtin_va_start(ap, last)
#define va_arg(ap, type) __builtin_va_arg(ap, type)
#define va_end(ap) __builtin_va_end(ap)

extern void report(long value);

struct Big { long a, b, c, d; };

long sum5(long a, long b, long c, long d, long e) { return a + b * 2 + c * 3 + d * 4 + e * 5; }
long fwd5(long a, long b, long c, long d, long e) { return sum5(a, b, c, d, e); }
long more5(long a, long b) { return sum5(a, b, 3, 4, 5); }
long swap5(long a, long b, long c, long d, long e) { return sum5(e, d, c, b, a); }

long big_sum(struct Big s) { return s.a + s.b * 10 + s.c * 100 + s.d * 1000; }
long big_fwd(struct Big s) { return big_sum(s); }
struct Big make_big(long x) { struct Big s; s.a = x; s.b = x + 1; s.c = x + 2; s.d = x + 3; return s; }
struct Big make_fwd(long x) { return make_big(x + 1); }

int vsum(int n, ...)
{
    va_list ap;
    int t = 0;
    int i;
    va_start(ap, n);
    for (i = 0; i < n; ++i) t = t * 3 + va_arg(ap, int);
    va_end(ap);
    return t;
}
int vfwd(int a, int b) { return vsum(2, a, b); }

double half(double x) { return x / 2.0; }
double half_fwd(double x) { return half(x + 3.0); }

long twice(long x) { return x * 2; }
long (*volatile chosen)(long) = twice;
long through(long x) { return chosen(x + 1); }

int main(void)
{
    struct Big b, m;
    report(fwd5(1, 2, 3, 4, 5));
    report(more5(10, 20));
    report(swap5(1, 2, 3, 4, 5));
    b.a = 1; b.b = 2; b.c = 3; b.d = 4;
    report(big_fwd(b));
    m = make_fwd(7);
    report(m.a + m.b * 10 + m.c * 100 + m.d * 1000);
    report(vfwd(5, 6));
    report((long)(half_fwd(9.0) * 4.0));
    report(through(20));
    return 0;
}
