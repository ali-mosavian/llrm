// RUN: llrm-cc %s -m64 -fno-pic -O3 -S -o /dev/stdout
// particle at m64 -O3: the unroll-and-jam of the 100-trip outer loop took count 3 and left a remainder copy of the nest (+433 B, +8.6% clk). A divisor of the trip count (2) leaves none.
// CHECK-LABEL: bench_particle proc
// CHECK-NOT: add eax, 3
// CHECK: bench_particle endp
extern void report(long value);

static short px[101], py[101], pz[101];
static short vx[101], vy[101], vz[101];

static void advance(short n)
{
    short i;

    for (i = 50 - n; i <= 50 + n; ++i) {
        px[i] = px[i] + vx[i];
        py[i] = py[i] + vy[i];
        pz[i] = pz[i] + vz[i];
    }
}

long bench_particle(void)
{
    short i, t, p;
    long s;

    for (i = 0; i <= 100; ++i) {
        p = i - 50;
        px[i] = p; py[i] = 2 * p; pz[i] = -p;
        vx[i] = p & 7; vy[i] = 3 - (p & 3); vz[i] = 1;
    }
    for (t = 1; t <= 100; ++t)
        advance(50);
    s = 0;
    for (i = 0; i <= 100; ++i)
        s = s + px[i] + 3L * py[i] + 7L * pz[i];
    return s;
}

int main(void)
{
    report(bench_particle());
    return 0;
}
