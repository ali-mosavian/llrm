// flags: -O2 -march=i486 | -Os -march=i486
/* Every float compare code, as a value, negated, in a branch and as a select, f32 and f64, over -1, 0, 1 and NaN:
   the pin's SETCC code expansion (LegalizeSetCCCondCode) must keep x86's oeq/une and every unordered result. */
extern void report(long value);

static volatile float vf[4];
static volatile double vd[4];
static volatile int zero;

static int __attribute__((noinline)) f_lt_v(float a, float b) { return a < b; }
static int __attribute__((noinline)) f_lt_n(float a, float b) { return !(a < b); }
static int __attribute__((noinline)) f_lt_b(float a, float b) { if (a < b) return 7; return 3; }
static int __attribute__((noinline)) f_lt_s(float a, float b) { return a < b ? 5 : 9; }
static int __attribute__((noinline)) f_le_v(float a, float b) { return a <= b; }
static int __attribute__((noinline)) f_le_n(float a, float b) { return !(a <= b); }
static int __attribute__((noinline)) f_le_b(float a, float b) { if (a <= b) return 7; return 3; }
static int __attribute__((noinline)) f_le_s(float a, float b) { return a <= b ? 5 : 9; }
static int __attribute__((noinline)) f_gt_v(float a, float b) { return a > b; }
static int __attribute__((noinline)) f_gt_n(float a, float b) { return !(a > b); }
static int __attribute__((noinline)) f_gt_b(float a, float b) { if (a > b) return 7; return 3; }
static int __attribute__((noinline)) f_gt_s(float a, float b) { return a > b ? 5 : 9; }
static int __attribute__((noinline)) f_ge_v(float a, float b) { return a >= b; }
static int __attribute__((noinline)) f_ge_n(float a, float b) { return !(a >= b); }
static int __attribute__((noinline)) f_ge_b(float a, float b) { if (a >= b) return 7; return 3; }
static int __attribute__((noinline)) f_ge_s(float a, float b) { return a >= b ? 5 : 9; }
static int __attribute__((noinline)) f_eq_v(float a, float b) { return a == b; }
static int __attribute__((noinline)) f_eq_n(float a, float b) { return !(a == b); }
static int __attribute__((noinline)) f_eq_b(float a, float b) { if (a == b) return 7; return 3; }
static int __attribute__((noinline)) f_eq_s(float a, float b) { return a == b ? 5 : 9; }
static int __attribute__((noinline)) f_ne_v(float a, float b) { return a != b; }
static int __attribute__((noinline)) f_ne_n(float a, float b) { return !(a != b); }
static int __attribute__((noinline)) f_ne_b(float a, float b) { if (a != b) return 7; return 3; }
static int __attribute__((noinline)) f_ne_s(float a, float b) { return a != b ? 5 : 9; }
static int __attribute__((noinline)) d_lt_v(double a, double b) { return a < b; }
static int __attribute__((noinline)) d_lt_n(double a, double b) { return !(a < b); }
static int __attribute__((noinline)) d_lt_b(double a, double b) { if (a < b) return 7; return 3; }
static int __attribute__((noinline)) d_lt_s(double a, double b) { return a < b ? 5 : 9; }
static int __attribute__((noinline)) d_le_v(double a, double b) { return a <= b; }
static int __attribute__((noinline)) d_le_n(double a, double b) { return !(a <= b); }
static int __attribute__((noinline)) d_le_b(double a, double b) { if (a <= b) return 7; return 3; }
static int __attribute__((noinline)) d_le_s(double a, double b) { return a <= b ? 5 : 9; }
static int __attribute__((noinline)) d_gt_v(double a, double b) { return a > b; }
static int __attribute__((noinline)) d_gt_n(double a, double b) { return !(a > b); }
static int __attribute__((noinline)) d_gt_b(double a, double b) { if (a > b) return 7; return 3; }
static int __attribute__((noinline)) d_gt_s(double a, double b) { return a > b ? 5 : 9; }
static int __attribute__((noinline)) d_ge_v(double a, double b) { return a >= b; }
static int __attribute__((noinline)) d_ge_n(double a, double b) { return !(a >= b); }
static int __attribute__((noinline)) d_ge_b(double a, double b) { if (a >= b) return 7; return 3; }
static int __attribute__((noinline)) d_ge_s(double a, double b) { return a >= b ? 5 : 9; }
static int __attribute__((noinline)) d_eq_v(double a, double b) { return a == b; }
static int __attribute__((noinline)) d_eq_n(double a, double b) { return !(a == b); }
static int __attribute__((noinline)) d_eq_b(double a, double b) { if (a == b) return 7; return 3; }
static int __attribute__((noinline)) d_eq_s(double a, double b) { return a == b ? 5 : 9; }
static int __attribute__((noinline)) d_ne_v(double a, double b) { return a != b; }
static int __attribute__((noinline)) d_ne_n(double a, double b) { return !(a != b); }
static int __attribute__((noinline)) d_ne_b(double a, double b) { if (a != b) return 7; return 3; }
static int __attribute__((noinline)) d_ne_s(double a, double b) { return a != b ? 5 : 9; }

#define GRID(F, V) do { long bits = 0; int k = 0; for (int i = 0; i < 4; i++) for (int j = 0; j < 4; j++, k++) bits |= (long)((F(V[i], V[j])) & 1) << k; report(bits); } while (0)
#define GRIDV(F, V) do { long bits = 0; int k = 0; for (int i = 0; i < 4; i++) for (int j = 0; j < 4; j++, k++) bits += (long)F(V[i], V[j]) * (k + 1); report(bits); } while (0)

int main(void)
{
    float nanf = (float)zero / (float)zero;
    double nand = (double)zero / (double)zero;
    vf[0] = -1; vf[1] = 0; vf[2] = 1; vf[3] = nanf;
    vd[0] = -1; vd[1] = 0; vd[2] = 1; vd[3] = nand;
    GRID(f_lt_v, vf);
    GRID(f_lt_n, vf);
    GRIDV(f_lt_b, vf);
    GRIDV(f_lt_s, vf);
    GRID(f_le_v, vf);
    GRID(f_le_n, vf);
    GRIDV(f_le_b, vf);
    GRIDV(f_le_s, vf);
    GRID(f_gt_v, vf);
    GRID(f_gt_n, vf);
    GRIDV(f_gt_b, vf);
    GRIDV(f_gt_s, vf);
    GRID(f_ge_v, vf);
    GRID(f_ge_n, vf);
    GRIDV(f_ge_b, vf);
    GRIDV(f_ge_s, vf);
    GRID(f_eq_v, vf);
    GRID(f_eq_n, vf);
    GRIDV(f_eq_b, vf);
    GRIDV(f_eq_s, vf);
    GRID(f_ne_v, vf);
    GRID(f_ne_n, vf);
    GRIDV(f_ne_b, vf);
    GRIDV(f_ne_s, vf);
    GRID(d_lt_v, vd);
    GRID(d_lt_n, vd);
    GRIDV(d_lt_b, vd);
    GRIDV(d_lt_s, vd);
    GRID(d_le_v, vd);
    GRID(d_le_n, vd);
    GRIDV(d_le_b, vd);
    GRIDV(d_le_s, vd);
    GRID(d_gt_v, vd);
    GRID(d_gt_n, vd);
    GRIDV(d_gt_b, vd);
    GRIDV(d_gt_s, vd);
    GRID(d_ge_v, vd);
    GRID(d_ge_n, vd);
    GRIDV(d_ge_b, vd);
    GRIDV(d_ge_s, vd);
    GRID(d_eq_v, vd);
    GRID(d_eq_n, vd);
    GRIDV(d_eq_b, vd);
    GRIDV(d_eq_s, vd);
    GRID(d_ne_v, vd);
    GRID(d_ne_n, vd);
    GRIDV(d_ne_b, vd);
    GRIDV(d_ne_s, vd);
    return 0;
}
