/* executed-instruction driver: calls LIBFN NV times over fixed vectors; KIND 1 = double(double,double), 2 = double(int32), 3 = double(int64), 4 = int(double). */
typedef unsigned long long u64; typedef long long i64;
static u64 s = 0x9E3779B97F4A7C15ull;
static u64 rnd(void) { s ^= s << 13; s ^= s >> 7; s ^= s << 17; return s; }
static u64 acc;
static u64 dbits(double d) { union { double d; u64 u; } x = { d }; return x.u; }
static double bitsd(u64 u) { union { double d; u64 u; } x; x.u = u; return x.d; }
static u64 pick(void) { u64 r = rnd(); return (r & 1 ? 1ull << 63 : 0) | ((1000ull + (r >> 1) % 47) << 52) | (rnd() & 0xFFFFFFFFFFFFFull); }
#if KIND == 1
double LIBFN(double, double); static double (*volatile fn)(double, double) = LIBFN;
#elif KIND == 2
double LIBFN(int); static double (*volatile fn)(int) = LIBFN;
#elif KIND == 3
double LIBFN(i64); static double (*volatile fn)(i64) = LIBFN;
#elif KIND == 6
int LIBFN(double, double); static int (*volatile fn)(double, double) = LIBFN;
#elif KIND == 5
float LIBFN(float, float); static float (*volatile fn)(float, float) = LIBFN;
#else
int LIBFN(double); static int (*volatile fn)(double) = LIBFN;
#endif
int main(void)
{
    for (int i = 0; i < NV; i++) {
#if KIND == 1
        acc += dbits(fn(bitsd(pick()), bitsd(pick())));
#elif KIND == 2
        acc += dbits(fn((int)(rnd() >> (rnd() % 32))));
#elif KIND == 3
        acc += dbits(fn((i64)(rnd() >> (rnd() % 64))));
#elif KIND == 6
        acc += (u64)fn(bitsd(pick()), bitsd(pick()));
#elif KIND == 5
        { union { float f; unsigned u; } a, b, r; a.u = (unsigned)(rnd() & 0x807FFFFF) | ((100u + rnd() % 50) << 23); b.u = (unsigned)(rnd() & 0x807FFFFF) | ((100u + rnd() % 50) << 23); r.f = fn(a.f, b.f); acc += r.u; }
#else
        { u64 r = rnd(); acc += (u64)fn(bitsd((r & 1 ? 1ull << 63 : 0) | ((1000ull + (r >> 1) % 31) << 52) | (rnd() & 0xFFFFFFFFFFFFFull))); }
#endif
    }
    return (int)(acc & 0x7f) == 0x7f;
}
#ifdef __riscv
void _start(void) { int r = main(); register long a0 __asm__("a0") = r; register long a7 __asm__("a7") = 93; __asm__ volatile("ecall" : : "r"(a0), "r"(a7)); for (;;) {} }
#else
void _start(void) { int r = main(); register long x0 __asm__("x0") = r; register long x8 __asm__("x8") = 93; __asm__ volatile("svc 0" : : "r"(x0), "r"(x8)); for (;;) {} }
#endif
void *memcpy(void *d, const void *s2, unsigned long n) { char *a = d; const char *b = s2; while (n--) *a++ = *b++; return d; }
void *memset(void *d, int c, unsigned long n) { char *a = d; while (n--) *a++ = c; return d; }
