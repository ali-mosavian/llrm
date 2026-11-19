static long got[32], n;
__attribute__((noinline)) void report(long v) { got[n++] = v; }
__attribute__((noinline)) long twice(long v) { return v * 2; }
__attribute__((noinline)) long run(long a, long b, long c)
{
    long m[8] __attribute__((aligned(16)));
    long x = twice(a), y = twice(b), z = twice(c);
    for (int j = 0; j < 8; ++j) m[j] = j * x;
    for (int j = 0; j < 8; ++j) report(m[j]);
    return x + y + z;
}
int main(void)
{
    if (run(1, 2, 3) != 12) return 2;
    for (int j = 0; j < 8; ++j) if (got[j] != j * 2) return 1;
    return 0;
}
