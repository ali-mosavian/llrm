/* Text screen through a pointer made of the machine's physical address, which the target's description
   names (PHYSICAL_TEXT_SCREEN): a flat target uses it as it is, real mode builds segment:offset. */
extern void report(long value);

#ifdef __386__
#define SCREEN ((unsigned char *)PHYSICAL_TEXT_SCREEN)
#else
#define SCREEN ((unsigned char far *)(((PHYSICAL_TEXT_SCREEN >> 4) << 16) | (PHYSICAL_TEXT_SCREEN & 15)))
#endif

static void fill(short ch, short at)
{
    short o;

    for (o = 0; o <= 3998; o += 2) {
        SCREEN[o] = (unsigned char)(ch + (o & 15));
        SCREEN[o + 1] = (unsigned char)at;
    }
}

static long checksum(void)
{
    long s = 0;
    short o;

    for (o = 0; o < 4000; o++)
        s += SCREEN[o];
    return s;
}

long bench_textfill(void)
{
    fill(65, 31);
    return checksum();
}

int main(void)
{
    report(bench_textfill());
    return 0;
}
