typedef unsigned long long u64;

u64 fibonacci64(unsigned count)
{
    u64 previous = 0;
    u64 current = 1;
    unsigned i;

    for (i = 0; i < count; ++i) {
        u64 next = previous + current;
        previous = current;
        current = next;
    }
    return previous;
}

/* F(93) is the largest Fibonacci number representable in u64. */
int fibonacci64Check(void)
{
    return fibonacci64(93) != 12200160415121876738ULL;
}

int main(void)
{
    return fibonacci64Check();
}
