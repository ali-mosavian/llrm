// flags: -O1 -ffixed-point | -O2 -ffixed-point | -Os -ffixed-point
// clang's fixed-point types: `*` is llvm.{s,u}mul.fix (.sat for _Sat), `<<` on a _Sat type is llvm.{s,u}shl.sat. 8 to 64
// bits, signed and unsigned, with results that saturate and that do not.
extern void report(long value);


#define MUL(T, U, ARR)                                       \
    do {                                                     \
        volatile T *p = ARR;                                 \
        int i, j;                                            \
        for (i = 0; i < 3; i++)                              \
            for (j = 0; j < 3; j++) {                        \
                T r = p[i] * p[j];                           \
                union { T f; U i; } u;                       \
                u.f = r;                                     \
                report((long)u.i);                           \
            }                                                \
    } while (0)

#define SHL(T, U, ARR)                                       \
    do {                                                     \
        volatile T *p = ARR;                                 \
        volatile int by = 3;                                 \
        int i;                                               \
        for (i = 0; i < 3; i++) {                            \
            T r = p[i] << by;                                \
            union { T f; U i; } u;                           \
            u.f = r;                                         \
            report((long)u.i);                               \
        }                                                    \
    } while (0)

_Sat short _Fract sf[3] = {0.5hr, -0.75hr, 0.99hr};
_Sat _Fract f[3] = {0.5r, -0.75r, 0.99r};
_Sat long _Fract lf[3] = {0.5lr, -0.75lr, 0.99lr};
_Sat unsigned short _Fract usf[3] = {0.5uhr, 0.75uhr, 0.99uhr};
_Sat unsigned _Fract uf[3] = {0.5ur, 0.75ur, 0.99ur};
_Sat unsigned long _Fract ulf[3] = {0.5ulr, 0.75ulr, 0.99ulr};
_Sat short _Accum sa[3] = {1.5hk, -2.25hk, 100.0hk};
_Sat _Accum a[3] = {1.5k, -2.25k, 100.0k};
_Sat long _Accum la[3] = {1.5lk, -2.25lk, 30000.0lk};
_Sat unsigned short _Accum usa[3] = {1.5uhk, 2.25uhk, 100.0uhk};
_Sat unsigned _Accum ua[3] = {1.5uk, 2.25uk, 100.0uk};
_Sat unsigned long _Accum ula[3] = {1.5ulk, 2.25ulk, 30000.0ulk};

int main(void)
{
    MUL(_Sat short _Fract, unsigned char, sf);
    MUL(_Sat _Fract, unsigned short, f);
    MUL(_Sat long _Fract, unsigned long, lf);
    MUL(_Sat unsigned short _Fract, unsigned char, usf);
    MUL(_Sat unsigned _Fract, unsigned short, uf);
    MUL(_Sat unsigned long _Fract, unsigned long, ulf);
    MUL(_Sat short _Accum, unsigned short, sa);
    MUL(_Sat _Accum, unsigned long, a);
    MUL(_Sat long _Accum, unsigned long long, la);
    MUL(_Sat unsigned short _Accum, unsigned short, usa);
    MUL(_Sat unsigned _Accum, unsigned long, ua);
    MUL(_Sat unsigned long _Accum, unsigned long long, ula);
    SHL(_Sat short _Fract, unsigned char, sf);
    SHL(_Sat _Fract, unsigned short, f);
    SHL(_Sat long _Fract, unsigned long, lf);
    SHL(_Sat unsigned _Fract, unsigned short, uf);
    SHL(_Sat short _Accum, unsigned short, sa);
    SHL(_Sat long _Accum, unsigned long long, la);
    SHL(_Sat unsigned long _Accum, unsigned long long, ula);
    return 0;
}
