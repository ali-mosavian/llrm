// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
extern void report(long value);

#define N 1024

static void sort(int *a, int lo, int hi)
{
    int pivot, i, j, t;

    if (lo >= hi) return;
    pivot = a[hi];
    i = lo;
    for (j = lo; j < hi; ++j)
        if (a[j] < pivot) {
            t = a[i]; a[i] = a[j]; a[j] = t;
            ++i;
        }
    t = a[i]; a[i] = a[hi]; a[hi] = t;
    sort(a, lo, i - 1);
    sort(a, i + 1, hi);
}

long bench_quicksort(unsigned short seed)
{
    int values[N];
    int i;
    unsigned short x = seed;
    long checksum = 0;

    for (i = 0; i < N; ++i) {
        x = x * 25173u + 13849u;
        values[i] = x & 0x7FFF;
    }
    sort(values, 0, N - 1);
    for (i = 1; i < N; ++i)
        if (values[i - 1] > values[i]) return -1;
    for (i = 0; i < N; ++i) checksum += (long)values[i] * ((i & 15) + 1);
    return checksum;
}

int main(void)
{
    report(bench_quicksort(1));
    return 0;
}
