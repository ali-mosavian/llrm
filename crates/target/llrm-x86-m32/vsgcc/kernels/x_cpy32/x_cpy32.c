extern void report(long value);
typedef long i32;
static i32 a[256], b[256];
long bench_x_cpy32(i32 n)
{
    i32 r, i; long sum = 0;
    for (i = 0; i < 256; ++i) a[i] = i * 7;
    for (r = 0; r < 64; ++r) { for (i = 0; i < n; ++i) b[i] = a[i]; sum += b[n - 1]; a[0] = (i32)sum; }
    return sum;
}
int main(void)
{
    report(bench_x_cpy32(32));
    return 0;
}
