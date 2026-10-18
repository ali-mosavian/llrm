// flags: -O2 -march=i486
// targets: x86-m32
/* The same rule at the flat target's width: a fill and a copy of 20000 bytes, made into memset and memcpy, store every byte. */
extern void report(long value);
unsigned char src[20000], dst[20000];

static void fill(void) { unsigned i; for (i = 0; i < 20000u; i++) dst[i] = 2; }
static void copy(void) { unsigned i; for (i = 0; i < 20000u; i++) dst[i] = src[i]; }

static unsigned long total(void)
{
    unsigned long sum = 0;
    unsigned i;
    for (i = 0; i < 20000u; i++) sum += dst[i];
    return sum;
}

int main(void)
{
    unsigned i;
    for (i = 0; i < 20000u; i++) src[i] = (unsigned char)((i & 3) + ((i >> 2) & 1));
    fill(); report((long)total());
    copy(); report((long)total());
    return 0;
}
