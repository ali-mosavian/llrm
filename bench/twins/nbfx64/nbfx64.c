// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* The Nib program's arithmetic in C: 64-bit products shifted right (floor), the divide as Nib's. */
extern void report(long value);

#define BODIES 6

static long pos_x[BODIES], pos_y[BODIES], vel_x[BODIES], vel_y[BODIES];

static long qmul(long a, long b)
{
    return (long)(((long long)a * b) >> 9);
}

static long tmul(long a, long b)
{
    return a < 0 ? 0 - qmul(0 - a, b) : qmul(a, b);
}

long bench_nbfx64(void)
{
    long delta_x, delta_y, wide_x, wide_y, dist2, falloff, acc_x, acc_y, sum;
    int body, other, step;

    for (body = 0; body < BODIES; ++body) {
        pos_x[body] = (body * 7 - 15) * 512L;
        pos_y[body] = (body * 5 - 12) * 512L;
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
                    wide_x = delta_x << 9;
                    wide_y = delta_y << 9;
                    dist2 = (long)(((long long)wide_x * wide_x) >> 18) + (long)(((long long)wide_y * wide_y) >> 18) + 262144L;
                    falloff = 262144L / (((dist2 / 262144L) + 1) << 9);
                    acc_x += tmul(delta_x, falloff);
                    acc_y += tmul(delta_y, falloff);
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
        sum += qmul((body + 1) * 512L, qmul(pos_x[body], 512L) + qmul(1536L, pos_y[body]) + qmul(2560L, vel_x[body]) + qmul(3584L, vel_y[body]));
    return qmul(sum, 262144L) / 512;
}

int main(void)
{
    report(bench_nbfx64());
    return 0;
}
