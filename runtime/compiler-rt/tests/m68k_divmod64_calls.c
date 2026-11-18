/* The call sites of the pair as llrm-cc makes them: a divide and a remainder of the same operands, written apart. */
unsigned long long f_upair(unsigned long long n, unsigned long long d, unsigned long long *r)
{
    unsigned long long q = n / d;
    *r = n % d;
    return q;
}
long long f_spair(long long n, long long d, long long *r)
{
    long long q = n / d;
    *r = n % d;
    return q;
}
