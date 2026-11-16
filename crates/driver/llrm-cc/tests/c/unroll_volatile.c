/* A counted loop that keeps its sum in a volatile: a chain through memory, run-time trips. */
unsigned down(int n)
{
    volatile unsigned s = 0;
    for (int i = n; i >= 0; i--) s = s + (((unsigned)i * 3u) ^ ((unsigned)i >> 2));
    return s;
}
