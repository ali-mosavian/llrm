/* Six-body Q9 fixed-point integrator, 1000 steps; long is 32-bit, / truncates toward zero. */
extern void report(long value);

#define BODIES 6
#define ONE 512L

static long pos_x[BODIES], pos_y[BODIES], vel_x[BODIES], vel_y[BODIES];

long bench_nbody_fixed(void)
{
    long delta_x, delta_y, dist2, falloff, acc_x, acc_y, sum;
    int body, other, step;

    for (body = 0; body < BODIES; ++body) {
        pos_x[body] = (body * 7 - 15) * ONE;
        pos_y[body] = (body * 5 - 12) * ONE;
        vel_x[body] = 0;
        vel_y[body] = 0;
    }
    for (step = 0; step < 1000; ++step) {
        for (body = 0; body < BODIES; ++body) {
            acc_x = 0;
            acc_y = 0;
            for (other = 0; other < BODIES; ++other) {
                if (other != body) {
                    delta_x = pos_x[other] - pos_x[body];
                    delta_y = pos_y[other] - pos_y[body];
                    dist2 = delta_x * delta_x + delta_y * delta_y + ONE * ONE;
                    falloff = ONE / (dist2 / (ONE * ONE) + 1);
                    acc_x += (delta_x * falloff) / ONE;
                    acc_y += (delta_y * falloff) / ONE;
                }
            }
            vel_x[body] += acc_x;
            vel_y[body] += acc_y;
            vel_x[body] -= vel_x[body] / 16;
            vel_y[body] -= vel_y[body] / 16;
        }
        for (body = 0; body < BODIES; ++body) {
            pos_x[body] += vel_x[body];
            pos_y[body] += vel_y[body];
        }
    }
    sum = 0;
    for (body = 0; body < BODIES; ++body)
        sum += (body + 1) * (pos_x[body] + 3 * pos_y[body] + 5 * vel_x[body] + 7 * vel_y[body]);
    return sum;
}

int main(void)
{
    report(bench_nbody_fixed());
    return 0;
}
