/* Text screen at B800:0000 through a far pointer. */
extern void report(long value);

static void fill(short ch, short at)
{
    unsigned char far *screen = (unsigned char far *)0xB8000000L;
    short o;

    for (o = 0; o <= 3998; o += 2) {
        screen[o] = (unsigned char)(ch + (o & 15));
        screen[o + 1] = (unsigned char)at;
    }
}

static long checksum(void)
{
    unsigned char far *screen = (unsigned char far *)0xB8000000L;
    long s = 0;
    short o;

    for (o = 0; o < 4000; o++)
        s += screen[o];
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
