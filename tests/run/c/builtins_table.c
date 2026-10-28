// flags: -O0 | -O2 | -Os
// The builtins the table lowers besides expect: constant_p is 0 or 1 (1 once a call is inlined), prefetch and unreachable do nothing, abs and strlen
// are the library's (defined below: the test links no C library).
extern void report(long value);

int abs(int x) { return x < 0 ? -x : x; }
unsigned strlen(const char *s)
{
    unsigned n = 0;
    while (s[n])
        n++;
    return n;
}

static int classify(int x)
{
    if (x > 100)
        __builtin_unreachable();
    __builtin_prefetch(&x, 0, 3);
    return (__builtin_constant_p(x) == 0 || __builtin_constant_p(x) == 1) + x - 1;
}

int main(void)
{
    report(__builtin_abs(-7));
    report((long)__builtin_strlen("hello"));
    report(classify(5));
    report(classify(40));
    return 0;
}
