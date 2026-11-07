// RUN: llrm-cc %s -m64 -O2 -S -o /dev/stdout
// fpbench at m64 -O2: four 6-element float arrays are 24 values for 16 registers, so the unroll of the 6-trip loops over them is not free of stack slots (UP1-REG). The price counted their accesses as free, 0 bytes added, and 12 loops became 38 `mulss`; the nest stays at most 30.
// CHECK-LABEL: bench_fpbench proc
// CHECK-MAX-30: mulss
// CHECK: bench_fpbench endp
/* Six-body float n-body; checksum = weighted sum of truncated (pos*64). */
extern void report(long value);

long bench_fpbench(short steps)
{
    float px[6], py[6], vx[6], vy[6];
    float dx, dy, d2, f, ax, ay;
    long acc;
    short step, b, o;

    for (b = 0; b < 6; ++b) {
        px[b] = (float)(b * 7 - 15);
        py[b] = (float)(b * 5 - 12);
        vx[b] = 0.0f;
        vy[b] = 0.0f;
    }

    for (step = 1; step <= steps; ++step) {
        for (b = 0; b < 6; ++b) {
            ax = 0.0f;
            ay = 0.0f;
            for (o = 0; o < 6; ++o) {
                if (o != b) {
                    dx = px[o] - px[b];
                    dy = py[o] - py[b];
                    d2 = dx * dx + dy * dy + 1.0f;
                    f = 1.0f / d2;
                    ax = ax + dx * f;
                    ay = ay + dy * f;
                }
            }
            vx[b] = vx[b] + ax;
            vy[b] = vy[b] + ay;
            vx[b] = vx[b] - vx[b] / 16.0f;
            vy[b] = vy[b] - vy[b] / 16.0f;
        }
        for (b = 0; b < 6; ++b) {
            px[b] = px[b] + vx[b];
            py[b] = py[b] + vy[b];
        }
    }

    acc = 0;
    for (b = 0; b < 6; ++b)
        acc += (long)(px[b] * 64.0f) * (b + 1) + (long)(py[b] * 64.0f) * (b + 7);
    return acc;
}
