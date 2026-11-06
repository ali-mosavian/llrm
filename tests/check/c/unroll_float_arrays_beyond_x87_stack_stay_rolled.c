// RUN: llrm-cc %s -m32 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// nbody at m32 -O2: four 4-element double arrays are 16 values for the x87 stack's 8 slots, so the unroll of the loops over them is not free of stack traffic (UP1-X87). The price counted their accesses as free, the nest was unrolled and its values kept across the inner loops: 20 `fxch` and 11 `fst` (381,454 clk against 362,987 rolled, gcc 363,879). Rolled it has 3.
// CHECK-LABEL: _bench_nbody proc
// CHECK-MAX-8: fxch
// CHECK: _bench_nbody endp
/* Four-body scalar kernel; result quantization is part of the oracle. */
extern void report(long value);

long bench_nbody(unsigned short steps)
{
    double x[4], y[4], vx[4], vy[4];
    unsigned short step, i, j;

    x[0] = -1.0; x[1] = 1.0; x[2] = 0.0; x[3] = 0.0;
    y[0] = 0.0; y[1] = 0.0; y[2] = -1.0; y[3] = 1.0;
    vx[0] = 0.0; vx[1] = 0.0; vx[2] = 0.0125; vx[3] = -0.0125;
    vy[0] = -0.0125; vy[1] = 0.0125; vy[2] = 0.0; vy[3] = 0.0;

    for (step = 0; step < steps; ++step) {
        for (i = 0; i < 4; ++i)
            for (j = (unsigned short)(i + 1); j < 4; ++j) {
                double dx = x[j] - x[i], dy = y[j] - y[i];
                double scale = 0.00001 / (dx * dx + dy * dy + 0.125);
                vx[i] += dx * scale; vy[i] += dy * scale;
                vx[j] -= dx * scale; vy[j] -= dy * scale;
            }
        for (i = 0; i < 4; ++i) {
            x[i] += vx[i];
            y[i] += vy[i];
        }
    }
    return (long)((x[0] + y[1] + x[2] + y[3]) * 1000000.0);
}

int main(void)
{
    report(bench_nbody(200));
    return 0;
}
