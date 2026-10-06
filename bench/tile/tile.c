// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
extern void report(long value);

static short tile[64][64];

long bench_tile(void)
{
    short w = 40, h = 25, dx = 7, dy = 61;
    long total = 0;
    short x, y;

    for (y = 0; y < h; ++y)
        for (x = 0; x < w; ++x)
            total += tile[(y + dy) & 63][(x + dx) & 63];
    return total;
}

int main(void)
{
    short x, y;

    for (y = 0; y < 64; ++y)
        for (x = 0; x < 64; ++x)
            tile[y][x] = (x * 3 + y * 5) & 255;
    report(bench_tile());
    return 0;
}
