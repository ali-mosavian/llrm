// flags: -O0 | -O3 -fno-inline-functions | -O3 -m32 -fno-inline-functions
// A loop whose address steps by an unknown stride is copied for a stride of 1 at -O3 (gcc's
// -fversion-loops-for-strides): each answer must be the one loop's, whichever stride it is run with.
extern void report(long value);

static short cells[256];

static void fill(short *a, short s, short n, short v)
{
    short i;
    for (i = 0; i < n; ++i)
        a[i * s] = v + i;
}

static long total(short n)
{
    long sum = 0;
    short i;
    for (i = 0; i < n; ++i)
        sum += (long)cells[i] * (i + 1);
    return sum;
}

static void clear(void)
{
    short i;
    for (i = 0; i < 256; ++i)
        cells[i] = 1000 + i;
}

int main(void)
{
    clear();
    fill(cells, 1, 40, 5);
    report(total(100));
    clear();
    fill(cells, 2, 40, 7);
    report(total(100));
    clear();
    fill(cells, 0, 40, 9);
    report(total(100));
    clear();
    fill(cells, 3, 0, 11);
    report(total(100));
    clear();
    fill(cells, 3, 70, 13);
    report(total(250));
    return 0;
}
