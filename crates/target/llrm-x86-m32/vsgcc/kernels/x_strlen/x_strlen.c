extern void report(long value);

static char buf[1024];
static int slen(const char *s) { const char *p = s; while (*p) ++p; return (int)(p - s); }
static void scopy(char *d, const char *s) { while ((*d++ = *s++) != 0) ; }
long bench_x_strlen(int n)
{
    int i, k; long total = 0; char tmp[1024];
    for (i = 0; i < 1000; ++i) buf[i] = (char)('a' + (i * 7) % 26);
    for (k = 0; k < n; ++k) {
        buf[500 + k % 400] = 0;
        total += slen(buf);
        scopy(tmp, buf);
        total += tmp[k % 100];
        buf[500 + k % 400] = 'z';
    }
    return total;
}

int main(void)
{
    report(bench_x_strlen(300));
    return 0;
}
