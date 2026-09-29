/* Callees: each stores what arrived where the harness reads it. */
#include "cases.h"

#define PARAM(n, T) T gv_##n; int gh_##n, gt_##n; \
    void CONV DIST p_##n(int h, T v, int t) { trash(); gh_##n = h; gv_##n = v; gt_##n = t; }
#define RESULT(n, T) T CONV DIST r_##n(void) { trash(); return in_##n; }
#define WIDEN(n, T) long gw_##n; \
    void CONV DIST w_##n(int h, T v, int t) { trash(); gw_##n = v; gh_##n = h; gt_##n = t; }

#ifdef AGGREGATE
AGGREGATES(PARAM) AGGREGATES(RESULT)
#else
SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
void CONV DIST r_void(void) { trash(); }

signed char gm_a; long gm_b; double gm_c; int gm_d; float gm_e; char far *gm_f; long double gm_h;
void CONV DIST p_mixed(signed char a, long b, double c, int d, float e, char far *f, long double h) {
    trash(); gm_a = a; gm_b = b; gm_c = c; gm_d = d; gm_e = e; gm_f = f; gm_h = h;
}

#ifdef VARIADIC
#define VSTORE(n, T, P) P gva_##n;
PROMOTED(VSTORE)
#undef VSTORE
int gva_n;
void CONV DIST p_variadic(int n, ...) {
    va_list ap;
    trash();
    gva_n = n;
    va_start(ap, n);
#define VREAD(n, T, P) gva_##n = va_arg(ap, P);
    PROMOTED(VREAD)
#undef VREAD
}
#endif
#endif
