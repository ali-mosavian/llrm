// flags: -O1 | -O2
extern void report(long value);

long long sub64(unsigned long a, unsigned long b)
{
    return (long long)a - (long long)b;
}

long long (*volatile through)(unsigned long, unsigned long) = sub64;

int main(void)
{
    long long r = through(5UL, 7UL);
    report((long)(r >> 32));
    report((long)r);
    r = through(7UL, 5UL);
    report((long)(r >> 32));
    report((long)r);
    r = through(0xFFFFFFFFUL, 1UL);
    report((long)(r >> 32));
    report((long)r);
    return 0;
}
