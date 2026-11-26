// Two calls of one loop with constant bounds: the copies share their code once the outliner runs.
extern void report(long value);

static short first[48], second[49];

static long mix(const short *p, int n)
{
    long total = 0;
    int i;
    for (i = 0; i < n; ++i)
        total = (total * 3 + p[i]) % 1000003;
    return total;
}

int main(void)
{
    int i;
    for (i = 0; i < 48; ++i)
    {
        first[i] = (short)(i * 7 + 1);
        second[i] = (short)(i * 5 + 2);
    }
    report(mix(first, 48));
    report(mix(second, 49));
    return 0;
}
