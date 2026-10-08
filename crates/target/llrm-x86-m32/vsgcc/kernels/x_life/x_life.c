extern void report(long value);

static unsigned char g[2][24][24];
long bench_x_life(int n)
{
    int x, y, gen, cur = 0, dx, dy; long alive = 0;
    for (y = 0; y < n; ++y) for (x = 0; x < n; ++x) g[0][y][x] = (unsigned char)(((x * 7 + y * 13) % 5) == 0);
    for (gen = 0; gen < 10; ++gen) {
        for (y = 1; y < n - 1; ++y) for (x = 1; x < n - 1; ++x) {
            int c = 0;
            for (dy = -1; dy <= 1; ++dy) for (dx = -1; dx <= 1; ++dx) c += g[cur][y + dy][x + dx];
            c -= g[cur][y][x];
            g[1 - cur][y][x] = (unsigned char)(c == 3 || (c == 2 && g[cur][y][x]));
        }
        cur = 1 - cur;
    }
    for (y = 0; y < n; ++y) for (x = 0; x < n; ++x) alive += g[cur][y][x];
    return alive;
}

int main(void)
{
    report(bench_x_life(24));
    return 0;
}
