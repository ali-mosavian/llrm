/* 40000 words (80000 bytes) in __huge memory, past 64K: copied, scrolled up and scrolled down.
   A huge pointer carries into its selector, so no `rep movs` may stand for these loops. */
extern void report(long value);

short __huge a[40000];
short __huge b[40000];

static long weigh(void)
{
    long t = 0;
    unsigned short i;
    for (i = 0; i < 40000u; ++i) t += (long)b[i] * ((i & 15) + 1);
    return t;
}

int main(void)
{
    unsigned short i;
    for (i = 0; i < 40000u; ++i) a[i] = i * 3 + 1;
    for (i = 0; i < 40000u; ++i) b[i] = a[i];
    report(weigh());
    for (i = 0; i < 39000u; ++i) b[i] = b[i + 1000];
    report(weigh());
    for (i = 39000u; i-- > 0;) b[i + 1000] = b[i];
    report(weigh());
    return 0;
}
