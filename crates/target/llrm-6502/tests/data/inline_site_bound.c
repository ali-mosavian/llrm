// A callee whose loop bound is the argument each call site passes as a constant.
extern void report(long value);

static unsigned short counts[64];

static long under(short n, short k)
{
    long sum = 0;
    short i;
    for (i = 0; i < n; i++)
        sum += i < k ? i * 3 : i + 7;
    return sum;
}

static long over(short n, short k)
{
    long sum = 0;
    short i;
    for (i = 0; i < n; i++)
        sum += i >= k ? i * 5 : 1;
    return sum;
}

static long mark(unsigned short limit)
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
    report(under(10, 4));
    report(under(10, -3));
    report(under(10, 0));
    report(under(10, 20));
    report(over(10, 4));
    report(over(10, 0));
    report(over(10, 20));
    report(over(0, 3));
    report(mark(100));
    report(mark(20));
    return 0;
}
