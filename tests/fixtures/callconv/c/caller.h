/* Callers: each passes the harness's inputs and keeps what came back. */
#include "cases.h"

/* The BCC caller is built with -DARMED -r-: no register variables, so SI
   and DI are the probe's alone from arm() to verify(). */
#ifdef ARMED
extern void far arm(void);
extern void far verify(const char *what);
#define ARM() arm()
#define VERIFY(what) verify(what)
#else
#define ARM()
#define VERIFY(what)
#endif

#define DECLARE(n, T) void CONV DIST p_##n(int h, T v, int t); T CONV DIST r_##n(void); T out_##n;
#define DECLARE_WIDE(n, T) void CONV DIST w_##n(int h, T v, int t); long ow_##n;
#ifdef AGGREGATE
AGGREGATES(DECLARE)
#else
SCALARS(DECLARE) NARROW(DECLARE_WIDE)
void CONV DIST r_void(void);
void CONV DIST p_mixed(signed char a, long b, double c, int d, float e, char far *f, long double h);
#ifdef VARIADIC
void CONV DIST p_variadic(int n, ...);
#endif
#endif

#define CALL(n, T) ARM(); p_##n(0x1111, in_##n, 0x2222); VERIFY("p_" #n); \
    ARM(); out_##n = r_##n(); VERIFY("r_" #n);
#define CALL_WIDE(n, T) ARM(); w_##n(0x1111, in_##n, 0x2222); VERIFY("w_" #n); \
    ARM(); ow_##n = r_##n(); VERIFY("rw_" #n);

#ifdef AGGREGATE
void far call_aggregates(void) {
    AGGREGATES(CALL)
}
#else
void far call_scalars(void) {
    SCALARS(CALL)
    NARROW(CALL_WIDE)
    ARM(); r_void(); VERIFY("r_void");
    ARM(); p_mixed(in_sc, in_l, in_d, in_i, in_f, in_fp, in_ld); VERIFY("p_mixed");
#ifdef VARIADIC
#define VARG(n, T, P) , in_##n
    ARM(); p_variadic(0x1111 PROMOTED(VARG)); VERIFY("p_variadic");
#undef VARG
#endif
}
#endif
