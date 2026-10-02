/* 100x201 longs (80400 bytes) in __huge memory, walked in memory order. */
extern void report(long value);

long __huge g[100][201];

long bench_grid2d(void)
{
    long t = 0;
    short r, c;

    for (r = 0; r < 100; ++r)
        for (c = 0; c < 201; ++c)
            g[r][c] = r * 1000L + c;
    for (r = 0; r < 100; ++r)
        for (c = 0; c < 201; ++c)
            t += g[r][c] & 255;
    return t;
}

int main(void)
{
    report(bench_grid2d());
    return 0;
}
