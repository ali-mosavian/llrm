// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
/* Integer Q8 Mandelbrot work count: deterministic, with no floating ambiguity. */
extern void report(long value);

long bench_mandel(void)
{
    short px, py, iteration;
    long work = 0;

    for (py = -12; py < 12; ++py)
        for (px = -16; px < 16; ++px) {
            long x = 0, y = 0;
            long cx = (long)px * 24 - 128;
            long cy = (long)py * 24;
            for (iteration = 0; iteration < 32; ++iteration) {
                long xx = (x * x) >> 8;
                long yy = (y * y) >> 8;
                if (xx + yy > 1024) break;
                y = ((x * y) >> 7) + cy;
                x = xx - yy + cx;
            }
            work += iteration;
        }
    return work;
}

int main(void)
{
    report(bench_mandel());
    return 0;
}
