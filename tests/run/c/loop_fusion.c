// flags: -O0 | -Omax | -Omax -m32
// Two adjacent loops of one trip count are one loop at -Omax (LLVM's LoopFuse): each answer must be the two loops'
// own, whichever way the second loop's reads fall against the first loop's writes.
extern void report(long value);

static short a[64], b[64];

static void clear(void)
{
    short i;
    for (i = 0; i < 64; i++) {
        a[i] = 100 + i;
        b[i] = 200 + i;
    }
}

static long total(short n)
{
    long sum = 0;
    short i;
    for (i = 0; i < n; i++)
        sum += b[i] + 3 * a[i];
    return sum;
}

/* b[i] reads the a[i] this trip wrote. */
static long same(short n)
{
    short i;
    clear();
    for (i = 0; i < n; i++)
        a[i] = i * 3;
    for (i = 0; i < n; i++)
        b[i] = a[i] + 1;
    return total(n);
}

/* b[i] reads a[i], written by the trip before. */
static long behind(short n)
{
    short i;
    clear();
    for (i = 0; i < n; i++)
        a[i + 1] = i * 3;
    for (i = 0; i < n; i++)
        b[i] = a[i] + 1;
    return total(n);
}

/* b[i] reads a[i + 1], which the first loop writes a trip later. */
static long ahead(short n)
{
    short i;
    clear();
    for (i = 0; i < n; i++)
        a[i] = i * 3;
    for (i = 0; i < n; i++)
        b[i] = a[i + 1] + 1;
    return total(n);
}

/* The second loop writes what the first reads a trip later. */
static long over(short n)
{
    short i;
    clear();
    for (i = 0; i < n; i++)
        b[i] = a[i + 1] + i;
    for (i = 0; i < n; i++)
        a[i] = 7;
    return total(n);
}

/* One trip more in the second loop. */
static long longer(short n)
{
    short i;
    clear();
    for (i = 0; i < n; i++)
        a[i] = i * 3;
    for (i = 0; i < n + 1; i++)
        b[i] = a[i] + 1;
    return total(n);
}

int main(void)
{
    report(same(10));
    report(same(0));
    report(same(1));
    report(behind(10));
    report(behind(0));
    report(ahead(10));
    report(ahead(1));
    report(over(10));
    report(over(0));
    report(longer(10));
    report(longer(0));
    return 0;
}
