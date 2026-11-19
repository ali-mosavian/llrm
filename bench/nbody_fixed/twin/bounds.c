// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Twin of nbody_fixed.nib with its semantics: every index checked against the array length, and every fixed-point operation as Nib defines
   it (language-spec 3.4): `*` the full-width product shifted right by the fraction (floor), `/` the dividend shifted left first (truncating),
   an integer literal in a Q expression converted to Q. Six-body Q9 fixed-point integrator, 1000 steps; long is 32-bit, / truncates toward zero. */
extern void report(long value);
extern void ebnd(void);
#define CK(i, n) ((unsigned)(i) < (n) ? (i) : (ebnd(), 0))

#define BODIES 6
#define ONE 512L

static long qmul(long a, long b, int fraction) { return (long)(((long long)a * b) >> fraction); }
static long qdiv(long a, long b, int fraction) { return (long)(((long long)a << fraction) / b); }
/* Nib's tmul: the product truncated toward zero. */
static long tmul(long a, long b) { return a < 0 ? 0 - qmul(0 - a, b, 9) : qmul(a, b, 9); }

static long pos_x[BODIES], pos_y[BODIES], vel_x[BODIES], vel_y[BODIES];

long bench_nbody_fixed(void)
{
    long delta_x, delta_y, wide_x, wide_y, dist2, falloff, acc_x, acc_y, sum;
    int body, other, step;

    for (body = 0; body < BODIES; ++body) {
        pos_x[CK(body, BODIES)] = (body * 7 - 15) * ONE;
        pos_y[CK(body, BODIES)] = (body * 5 - 12) * ONE;
        vel_x[CK(body, BODIES)] = 0;
        vel_y[CK(body, BODIES)] = 0;
    }
    for (step = 0; step < 1000; ++step) {
        for (body = 0; body < BODIES; ++body) {
            acc_x = 0;
            acc_y = 0;
            for (other = 0; other < BODIES; ++other) {
                if (other != body) {
                    delta_x = pos_x[CK(other, BODIES)] - pos_x[CK(body, BODIES)];
                    delta_y = pos_y[CK(other, BODIES)] - pos_y[CK(body, BODIES)];
                    wide_x = delta_x * ONE; /* P(delta): Q18 */
                    wide_y = delta_y * ONE;
                    dist2 = qmul(wide_x, wide_x, 18) + qmul(wide_y, wide_y, 18) + ONE * ONE; /* the literal 1 as Q18 */
                    falloff = qdiv(ONE, (dist2 / (ONE * ONE) + 1) * ONE, 9); /* 1 / Q(i32(dist2) + 1) */
                    acc_x += tmul(delta_x, falloff);
                    acc_y += tmul(delta_y, falloff);
                }
            }
            vel_x[CK(body, BODIES)] += acc_x;
            vel_y[CK(body, BODIES)] += acc_y;
            vel_x[CK(body, BODIES)] -= qdiv(vel_x[CK(body, BODIES)], 16 * ONE, 9);
            vel_y[CK(body, BODIES)] -= qdiv(vel_y[CK(body, BODIES)], 16 * ONE, 9);
        }
        for (body = 0; body < BODIES; ++body) {
            pos_x[CK(body, BODIES)] += vel_x[CK(body, BODIES)];
            pos_y[CK(body, BODIES)] += vel_y[CK(body, BODIES)];
        }
    }
    sum = 0;
    for (body = 0; body < BODIES; ++body)
        sum += qmul((body + 1) * ONE,
                    pos_x[CK(body, BODIES)] + qmul(3 * ONE, pos_y[CK(body, BODIES)], 9) + qmul(5 * ONE, vel_x[CK(body, BODIES)], 9) +
                        qmul(7 * ONE, vel_y[CK(body, BODIES)], 9),
                    9);
    return qmul(sum, 512 * ONE, 9) / ONE; /* i32(sum * 512) */
}

void ebnd(void) {}

int main(void)
{
    report(bench_nbody_fixed());
    return 0;
}
