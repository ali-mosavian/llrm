extern void report(long value);

static unsigned char in[512], out[1100];
long bench_x_rle(int n)
{
    int i = 0, o = 0; long sum = 0;
    for (i = 0; i < n; ++i) in[i] = (unsigned char)((i / 7) * 3 + (i % 11 == 0));
    i = 0;
    while (i < n) { int run = 1; while (i + run < n && run < 255 && in[i + run] == in[i]) ++run; out[o++] = (unsigned char)run; out[o++] = in[i]; i += run; }
    for (i = 0; i < o; ++i) sum = sum * 5 + out[i];
    return sum + o;
}

int main(void)
{
    report(bench_x_rle(500));
    return 0;
}
