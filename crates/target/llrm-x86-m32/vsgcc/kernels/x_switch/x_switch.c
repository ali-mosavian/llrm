extern void report(long value);

static int step(int s, int c)
{
    switch (s) {
    case 0: return c < 64 ? 1 : (c < 128 ? 2 : 0);
    case 1: return c & 1 ? 3 : 0;
    case 2: return c > 200 ? 4 : 1;
    case 3: return (c & 6) == 6 ? 2 : 5;
    case 4: return c ^ 3 ? 0 : 5;
    default: return c < 10 ? 0 : 3;
    }
}
long bench_x_switch(int n)
{
    int i, s = 0; long sum = 0; unsigned r = 9;
    for (i = 0; i < n; ++i) { r = r * 1103515245u + 12345u; s = step(s, (int)((r >> 16) & 255)); sum += s * (i & 3); }
    return sum;
}

int main(void)
{
    report(bench_x_switch(1500));
    return 0;
}
