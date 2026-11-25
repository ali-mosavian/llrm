// A recursion runs its multiply once per depth, not once.
extern void report(long);

static long descend(long n, long *total)
{
    long cell = n * 2;

    if (n == 0) return *total;
    *total += cell;
    return n - descend(n - 1, total) + cell;
}

static long scale(long a, long b)
{
    return a * b;
}

int main(void)
{
    long total = 0;

    report(descend(10, &total));
    report(scale(total, total + 5));
    return 0;
}
