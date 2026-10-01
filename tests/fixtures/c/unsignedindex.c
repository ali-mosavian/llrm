/* An unsigned index past 16383 into a far word array: the scaled offset passes
   32767, which a signed multiply may not claim to hold. */
long sum(int __far *a, unsigned n)
{
    long s = 0;
    unsigned i;
    for (i = 0; i < n; i++)
        s += a[i];
    return s;
}
