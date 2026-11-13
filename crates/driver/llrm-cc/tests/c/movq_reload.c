typedef unsigned long long u64;
extern double d(u64 x);
extern float f(u64 x);

u64 g(u64 x)
{
    union { double d; u64 u; } p;
    union { float f; unsigned u; } q;
    p.d = d(x);
    q.f = f(x);
    return p.u ^ q.u;
}
