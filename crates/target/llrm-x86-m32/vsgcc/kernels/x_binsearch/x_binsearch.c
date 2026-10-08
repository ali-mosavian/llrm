extern void report(long value);

static int a[512];
long bench_x_binsearch(int n)
{
    int i, lo, hi, mid, key; long found = 0; unsigned s = 5;
    for (i = 0; i < 512; ++i) a[i] = i * 3 + 1;
    for (i = 0; i < n; ++i) {
        s = s * 1103515245u + 12345u; key = (int)((s >> 16) % 1600);
        lo = 0; hi = 511;
        while (lo <= hi) { mid = (lo + hi) >> 1; if (a[mid] < key) lo = mid + 1; else if (a[mid] > key) hi = mid - 1; else { found += mid; break; } }
    }
    return found;
}

int main(void)
{
    report(bench_x_binsearch(2000));
    return 0;
}
