/* Integer Q8 Mandelbrot work count: deterministic, with no floating ambiguity. */
unsigned long bench_mandel(short xOffset)
{
    short px, py, iteration;
    unsigned long work = 0;

    for (py = -12; py < 12; ++py)
        for (px = -16; px < 16; ++px) {
            long x = 0, y = 0;
            long cx = (long)px * 24 - 128 + xOffset;
            long cy = (long)py * 24;
            for (iteration = 0; iteration < 32; ++iteration) {
                long xx = (x * x) >> 8;
                long yy = (y * y) >> 8;
                if (xx + yy > 1024) break;
                y = ((x * y) >> 7) + cy;
                x = xx - yy + cx;
            }
            work += (unsigned short)iteration;
        }
    return work;
}
