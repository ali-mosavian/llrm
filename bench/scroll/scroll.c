/* Scroll an 80x25 text screen up a row, blank the bottom row, scroll it back down, and keep a copy
   in a back buffer: fifty times, then a weighted sum of the copy. */
extern void report(long value);

static short screen[2000];
static short back[2000];

long bench_scroll(void)
{
    long total = 0;
    short r, i;

    for (r = 0; r < 50; ++r) {
        for (i = 0; i < 1920; ++i) screen[i] = screen[i + 80];
        for (i = 1920; i < 2000; ++i) screen[i] = 0x0720;
        for (i = 1919; i >= 0; --i) screen[i + 80] = screen[i];
        for (i = 0; i < 2000; ++i) back[i] = screen[i];
    }
    for (i = 0; i < 2000; ++i) total += (long)back[i] * ((i & 15) + 1);
    return total;
}

int main(void)
{
    short i;

    for (i = 0; i < 2000; ++i) screen[i] = ((i * 7) & 255) | 0x0700;
    report(bench_scroll());
    return 0;
}
