// flags: -O2 -march=i486
// targets: x86-m16
/* Fills, copies and moves through huge pointers, of 0 to 70000 bytes, starting anywhere in a 64K segment and crossing its
   end: the runs of a loop made into rep stos and rep movs end where the offset wraps, and the selector steps. Each
   line is a weighted sum of the bytes around the operation, so a byte missed, doubled or put elsewhere shows. A move
   is a copy within big, its direction the loop's; a displacement the compiler can see keeps it a move and not a smear.
   Words and dwords are aligned to their size, as a huge array's elements are. */
extern void report(long value);
unsigned char __huge big[300000];
unsigned char __huge src[200000];
unsigned char nbuf[3000];

static unsigned long around(unsigned long lo, unsigned long hi)
{
    unsigned long sum = 0, i;
    for (i = lo; i < hi; i++) sum += (unsigned long)big[i] * (i % 251 + 1);
    return sum;
}

static unsigned long nearsum(unsigned long hi)
{
    unsigned long sum = 0, i;
    for (i = 0; i < hi + 3 && i < 3000; i++) sum += (unsigned long)nbuf[i] * (i % 251 + 1);
    return sum;
}

static void __attribute__((noinline)) fill(unsigned long start, unsigned long n, unsigned char v)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[start + i] = v;
}

static void __attribute__((noinline)) fill16(unsigned long start, unsigned long n, unsigned short v)
{
    unsigned short __huge *p = (unsigned short __huge *)(big + start);
    unsigned long i;
    for (i = 0; i < n; i++) p[i] = v;
}

static void __attribute__((noinline)) fill32(unsigned long start, unsigned long n, unsigned long v)
{
    unsigned long __huge *p = (unsigned long __huge *)(big + start);
    unsigned long i;
    for (i = 0; i < n; i++) p[i] = v;
}

static void __attribute__((noinline)) copy(unsigned long to, unsigned long from, unsigned long n)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[to + i] = src[from + i];
}

static void __attribute__((noinline)) tonear(unsigned long from, unsigned long n)
{
    unsigned long i;
    for (i = 0; i < n; i++) nbuf[i] = big[from + i];
}

static void __attribute__((noinline)) fromnear(unsigned long to, unsigned long n)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[to + i] = nbuf[i];
}

static void __attribute__((noinline)) up_1(unsigned long base, unsigned long n)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[base + i] = big[base + 1ul + i];
}

static void __attribute__((noinline)) down_1(unsigned long base, unsigned long n)
{
    unsigned long i;
    for (i = n; i > 0; i--) big[base + 1ul + i - 1] = big[base + i - 1];
}

static void __attribute__((noinline)) up_4(unsigned long base, unsigned long n)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[base + i] = big[base + 4ul + i];
}

static void __attribute__((noinline)) down_4(unsigned long base, unsigned long n)
{
    unsigned long i;
    for (i = n; i > 0; i--) big[base + 4ul + i - 1] = big[base + i - 1];
}

static void __attribute__((noinline)) up_65500(unsigned long base, unsigned long n)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[base + i] = big[base + 65500ul + i];
}

static void __attribute__((noinline)) down_65500(unsigned long base, unsigned long n)
{
    unsigned long i;
    for (i = n; i > 0; i--) big[base + 65500ul + i - 1] = big[base + i - 1];
}

int main(void)
{
    unsigned long i;
    for (i = 0; i < 200000; i++) src[i] = (unsigned char)(i * 7 + 3);
    for (i = 0; i < 3000; i++) nbuf[i] = (unsigned char)(i * 5 + 1);
    fill(0ul, 65535ul, 1); report((long)around(0ul, 65538ul));
    fill(0ul, 65536ul, 2); report((long)around(0ul, 65539ul));
    fill(0ul, 65537ul, 3); report((long)around(0ul, 65540ul));
    fill(1ul, 65536ul, 4); report((long)around(0ul, 65540ul));
    fill(65530ul, 20ul, 5); report((long)around(65527ul, 65553ul));
    fill(65535ul, 65537ul, 6); report((long)around(65532ul, 131075ul));
    fill(100000ul, 1ul, 7); report((long)around(99997ul, 100004ul));
    fill(3ul, 0ul, 8); report((long)around(0ul, 6ul));
    fill(131070ul, 140ul, 9); report((long)around(131067ul, 131213ul));
    fill16(0ul, 32768ul, 4660); report((long)around(0ul, 65539ul));
    fill16(0ul, 32769ul, 22136); report((long)around(0ul, 65541ul));
    fill16(65534ul, 40ul, 39612); report((long)around(65531ul, 65617ul));
    fill16(1001ul, 10ul, 3085); report((long)around(998ul, 1024ul));
    fill16(65500ul, 200ul, 16705); report((long)around(65497ul, 65903ul));
    fill32(0ul, 16384ul, 16909060ul); report((long)around(0ul, 65539ul));
    fill32(0ul, 16385ul, 84281096ul); report((long)around(0ul, 65543ul));
    fill32(65532ul, 40ul, 168496141ul); report((long)around(65529ul, 65695ul));
    fill32(131068ul, 3ul, 287454020ul); report((long)around(131065ul, 131083ul));
    copy(0ul, 0ul, 65535ul); report((long)around(0ul, 65538ul));
    copy(0ul, 0ul, 65536ul); report((long)around(0ul, 65539ul));
    copy(0ul, 0ul, 65537ul); report((long)around(0ul, 65540ul));
    copy(65535ul, 1ul, 65537ul); report((long)around(65532ul, 131075ul));
    copy(65530ul, 65520ul, 40ul); report((long)around(65527ul, 65573ul));
    copy(70000ul, 3ul, 2ul); report((long)around(69997ul, 70005ul));
    copy(131068ul, 65534ul, 130ul); report((long)around(131065ul, 131201ul));
    tonear(65500ul, 2000ul); report((long)nearsum(2000ul));
    tonear(0ul, 3000ul); report((long)nearsum(3000ul));
    tonear(131000ul, 100ul); report((long)nearsum(100ul));
    fromnear(65500ul, 2000ul); report((long)around(65497ul, 67503ul));
    fromnear(0ul, 3000ul); report((long)around(0ul, 3003ul));
    fromnear(131022ul, 100ul); report((long)around(131019ul, 131125ul));
    up_1(0ul, 65537ul); report((long)around(0ul, 65541ul));
    up_4(65530ul, 40ul); report((long)around(65527ul, 65577ul));
    up_65500(500ul, 65536ul); report((long)around(497ul, 131539ul));
    up_1(65534ul, 70000ul); report((long)around(65531ul, 135538ul));
    down_1(0ul, 65537ul); report((long)around(0ul, 65541ul));
    down_4(65530ul, 40ul); report((long)around(65527ul, 65577ul));
    down_65500(500ul, 65536ul); report((long)around(497ul, 131539ul));
    down_1(65534ul, 70000ul); report((long)around(65531ul, 135538ul));
    return 0;
}
