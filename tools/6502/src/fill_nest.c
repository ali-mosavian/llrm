// flags: -O2 -march=i486 | -O2 -march=i486 -m32 | -Os -march=i486 -m32
/* A nest that fills a local's rows with one value: every row, not the first alone.
   `rep stos` counts cx down to 0, and its count was set once, outside the nest. */
extern void report(long value);
long words(void)
{
    short t[64][64];
    short y, x;
    long s = 0;
    for (y = 0; y < 64; ++y)
        for (x = 0; x < 64; ++x) t[y][x] = 4660;
    for (y = 0; y < 64; ++y)
        s += t[y][0] + t[y][63];
    return s;
}
long bytes(void)
{
    char t[64][64];
    short y, x;
    long s = 0;
    for (y = 0; y < 64; ++y)
        for (x = 0; x < 64; ++x) t[y][x] = 65;
    for (y = 0; y < 64; ++y)
        s += t[y][0] + t[y][63];
    return s;
}
int main(void) { report(words()); report(bytes()); return 0; }
