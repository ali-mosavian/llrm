// flags: -O2 --cpu 486 | -Os --cpu 486 | -O2 --cpu 486 -fsanitize=bounds
/* A string op sets ES to DGROUP; a far pointer read after it must still hold its own selector. */
extern void report(long value);
short __far far_a[100];
short near_a[100], near_b[100];

static long walk(short __far *p)
{
    long s = 0;
    short r, i;

    for (r = 0; r < 100; ++r) {
        for (i = 0; i < 100; ++i) near_b[i] = near_a[i] + r;
        s += p[r] + near_b[r];
        for (i = 0; i < 100; ++i) near_a[i] = near_b[i];
    }
    return s;
}

int main(void)
{
    short i;

    for (i = 0; i < 100; ++i) { near_a[i] = i; far_a[i] = i * 3; }
    report(walk(far_a));
    return 0;
}
