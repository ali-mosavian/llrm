// flags: -O2 -march=i486
// targets: x86-m16
/* Copies between __huge arrays that cross a 64K segment. The loop's far-pointer phi was numbered 31, the pin's ES: its
   selector copy was taken for a segment load and dropped, so the value was read before it was defined (huge_runs). */
extern void report(long value);
unsigned char __huge big[140000];
unsigned char __huge src[140000];

static void __attribute__((noinline)) copy(unsigned long to, unsigned long from, unsigned long n)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[to + i] = src[from + i];
}

static unsigned long __attribute__((noinline)) around(unsigned long lo, unsigned long hi)
{
    unsigned long sum = 0, i;
    for (i = lo; i < hi; i++) sum += (unsigned long)big[i] * (i % 251 + 1);
    return sum;
}

int main(void)
{
    unsigned long i;
    for (i = 0; i < 140000; i++) src[i] = (unsigned char)(i * 7 + 3);
    copy(0ul, 0ul, 65535ul);
    report((long)around(0ul, 65538ul));
    copy(65535ul, 1ul, 65537ul);
    report((long)around(65532ul, 131075ul));
    copy(65530ul, 65520ul, 40ul);
    report((long)around(65527ul, 65573ul));
    return 0;
}
