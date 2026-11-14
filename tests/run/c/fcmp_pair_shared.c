// flags: -O2 -march=i486 | -Os -march=i486
// bar: not comparable (SDCC double is 32 bits: it computes a different program)
/* All six predicates of one pair in one expression, f32 and f64, over -1, 0, 1, NaN, +inf and -0: the
   x87 status word is tested once per predicate (DEVIATE X87-STATUS); NaN must clear every ordered one. */
extern void report(long value);

static volatile float vf[6];
static volatile double vd[6];
static volatile int zero;

static unsigned __attribute__((noinline)) six_f(float a, float b)
{
    return (a < b) + 2 * (a == b) + 4 * (a > b) + 8 * (a != b) + 16 * (a <= b) + 32 * (a >= b);
}

static unsigned __attribute__((noinline)) six_d(double a, double b)
{
    return (a < b) + 2 * (a == b) + 4 * (a > b) + 8 * (a != b) + 16 * (a <= b) + 32 * (a >= b);
}

int main(void)
{
    float nanf = (float)zero / (float)zero;
    double nand = (double)zero / (double)zero;
    vf[0] = -1; vf[1] = 0; vf[2] = 1; vf[3] = nanf; vf[4] = __builtin_inff(); vf[5] = -0.0f;
    vd[0] = -1; vd[1] = 0; vd[2] = 1; vd[3] = nand; vd[4] = __builtin_inf(); vd[5] = -0.0;
    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            report(six_f(vf[i], vf[j]));
    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            report(six_d(vd[i], vd[j]));
    return 0;
}
