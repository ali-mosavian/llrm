// RUN: llrm-cc %s -m16 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// m16 nbody -O2 (M16-NBODY-O2): xi and yi were loaded before the inner loop,
// copied by each read and popped after it (`fld st(i)` 4 clocks on the 486, a
// load 3; the entry's `fld`/`fstp` pair 6 more). Each read is a memory operand:
// 368,427 clk to 361,227, 8 bytes smaller. No pop after the loop.
// CHECK-LABEL: _bench_nbody@3 proc far
// CHECK: fsubr qword ptr
// CHECK-NOT: fstp st(1)
// CHECK: _bench_nbody@3 endp
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
