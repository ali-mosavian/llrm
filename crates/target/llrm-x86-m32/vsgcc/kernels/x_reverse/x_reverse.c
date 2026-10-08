extern void report(long value);

static char s[512];
static void rev(char *p, int len) { int i; for (i = 0; i < len / 2; ++i) { char t = p[i]; p[i] = p[len - 1 - i]; p[len - 1 - i] = t; } }
long bench_x_reverse(int n)
{
    int i, k; long sum = 0;
    for (i = 0; i < 400; ++i) s[i] = (char)(i * 13 + 5);
    for (k = 0; k < n; ++k) { rev(s, 400 - (k & 15)); sum += s[k & 255]; }
    return sum;
}

int main(void)
{
    report(bench_x_reverse(400));
    return 0;
}
