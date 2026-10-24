// flags: -O2 -march=i486
// targets: x86-m16
/* Fills of a __huge array that cross its 64K segment: the run of the loop made into rep stos ends where the offset
   wraps, and the selector steps. The sums show a byte missed, doubled or put elsewhere. */
extern void report(long value);
unsigned char __huge big[140000];

static void __attribute__((noinline)) fill(unsigned long start, unsigned long n, unsigned char v)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[start + i] = v;
}

static unsigned long __attribute__((noinline)) around(unsigned long lo, unsigned long hi)
{
    unsigned long sum = 0, i;
    for (i = lo; i < hi; i++) sum += (unsigned long)big[i] * (i % 251 + 1);
    return sum;
}

int main(void)
{
    fill(65000, 2000, 7);
    report(around(64990, 67020));
    fill(0, 70000, 3);
    report(around(65530, 65545));
    fill(130000, 9000, 9);
    report(around(129990, 139999));
    return 0;
}
