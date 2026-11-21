/* A void function that ends in a call whose result it ignores. */
extern void sink(int *);

__attribute__((noinline)) static int *find(int *s, int n)
{
    sink(s);
    return s + n;
}

__attribute__((noinline)) static double half(int *s)
{
    sink(s);
    return *s / 2.0;
}

int *used_find(int *s) { return find(s, s[1]); }
double used_half(int *s) { return half(s); }

void ignore_find(int *s) { find(s, s[2]++); }
void ignore_half(int *s) { half(s); }
