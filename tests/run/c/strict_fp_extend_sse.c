// flags: -O2
// targets: x86-m64
/* A constrained float or double widened to long double, and a long double narrowed (FENV_ACCESS makes them STRICT_FP_EXTEND / STRICT_FP_ROUND between
   an xmm value and the x87's): selection had no form for the crossing, `CannotSelect("StrictFpExtend")`. The value goes through a stack cell. */
#pragma STDC FENV_ACCESS ON
extern void report(long value);

static long double __attribute__((noinline)) widen(double x) { return x; }
static long double __attribute__((noinline)) widen_float(float x) { return x; }
static double __attribute__((noinline)) narrow(long double x) { return (double)x; }
static float __attribute__((noinline)) narrow_float(long double x) { return (float)x; }

int main(void)
{
    report((long)(widen(2.5) * 4));
    report((long)(widen_float(1.25f) * 8));
    report((long)(narrow(7.75L) * 4));
    report((long)(narrow_float(0.5L) * 10));
    return 0;
}
