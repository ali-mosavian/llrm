/* wasm32 executed-instruction driver: calls LIBFN NV times; KIND 1 = double(double,double), 2 = double(int), 3 = double(long long). Data goes through bit casts so the loop itself has no float arithmetic. */
typedef unsigned long long u64;
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
#else
double LIBFN(long long); static double (*volatile fn)(long long) = LIBFN;
#endif
int go(void)
{
    for (int i = 0; i < NV; i++) {
#if KIND == 1
        acc += dbits(fn(bitsd(pick()), bitsd(pick())));
#elif KIND == 2
        acc += dbits(fn((int)(rnd() >> (rnd() % 32))));
#else
        acc += dbits(fn((long long)(rnd() >> (rnd() % 64))));
#endif
    }
    return (int)(acc & 0x7f);
}
