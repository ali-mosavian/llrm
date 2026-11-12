// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Twin of nbody_fixed.nib with its semantics: every index checked against the array length. Six-body Q9 fixed-point integrator, 1000 steps; long is 32-bit, / truncates toward zero. */
extern void report(long value);
extern void ebnd(void);
#define CK(i, n) ((unsigned)(i) < (n) ? (i) : (ebnd(), 0))

#define BODIES 6
#define ONE 512L

static long pos_x[BODIES], pos_y[BODIES], vel_x[BODIES], vel_y[BODIES];

long bench_nbody_fixed(void)
{
    long delta_x, delta_y, dist2, falloff, acc_x, acc_y, sum;
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
                    dist2 = delta_x * delta_x + delta_y * delta_y + ONE * ONE;
                    falloff = ONE / (dist2 / (ONE * ONE) + 1);
                    acc_x += (delta_x * falloff) / ONE;
                    acc_y += (delta_y * falloff) / ONE;
                }
            }
            vel_x[CK(body, BODIES)] += acc_x;
            vel_y[CK(body, BODIES)] += acc_y;
            vel_x[CK(body, BODIES)] -= vel_x[CK(body, BODIES)] / 16;
            vel_y[CK(body, BODIES)] -= vel_y[CK(body, BODIES)] / 16;
        }
        for (body = 0; body < BODIES; ++body) {
            pos_x[CK(body, BODIES)] += vel_x[CK(body, BODIES)];
            pos_y[CK(body, BODIES)] += vel_y[CK(body, BODIES)];
        }
    }
    sum = 0;
    for (body = 0; body < BODIES; ++body)
        sum += (body + 1) * (pos_x[CK(body, BODIES)] + 3 * pos_y[CK(body, BODIES)] + 5 * vel_x[CK(body, BODIES)] + 7 * vel_y[CK(body, BODIES)]);
    return sum;
}

void ebnd(void) {}

int main(void)
{
    report(bench_nbody_fixed());
    return 0;
}
