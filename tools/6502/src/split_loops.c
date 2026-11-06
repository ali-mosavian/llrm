// flags: -O0 | -O3 | -O3 -m32
// A test of the counter against an invariant in the loop (`i < k`, `i <= 31`, `i >= k`) is split out into two loops
// at -O3 (gcc's -fsplit-loops): each answer must be the unsplit loop's, whatever side of the loop the limit falls.
extern void report(long value);

static unsigned short counts[64];

static long below(short n, short k)
{
    long sum = 0;
    short i;
    for (i = 0; i < n; i++)
        sum += i < k ? i * 3 : i + 7;
    return sum;
}

static long from(short n, short k)
{
    long sum = 0;
    short i;
    for (i = 0; i < n; i++)
        sum += i >= k ? i * 5 : 1;
    return sum;
}

static long marks(unsigned short limit)
{
    unsigned short i, j, count = 0;
    for (i = 2; i < limit; i++)
        if (i <= 31)
            for (j = i * i; j < limit; j += i)
                counts[j & 63]++;
        else
            count++;
    return count + counts[5] + counts[17];
}

int main(void)
{
    report(below(10, 4));
    report(below(10, 0));
    report(below(10, 20));
    report(below(10, -3));
    report(from(10, 4));
    report(from(10, 0));
    report(from(10, 20));
    report(from(0, 3));
    report(marks(100));
    report(marks(20));
    return 0;
}
