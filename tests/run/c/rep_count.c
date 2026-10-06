// flags: -Os --cpu 486 | -O2 --cpu 486 --target x86-code32 | -Os --cpu 486 --target x86-code32
/* A `rep stosb` in a loop counts cx down to 0: its `mov cx, n` stayed outside the loop, so only the first
   row of each pass was cleared. */
extern void report(long value);
char g[16][54];
long bench(void)
{
    short i, j;
    long s = 0;
    for (i = 0; i < 16; ++i)
        for (j = 0; j < 54; ++j) g[i][j] = 1;
    for (i = 0; i < 16; ++i)
        for (j = 0; j < 16; ++j) g[i][j] = 0;
    for (i = 0; i < 16; ++i)
        for (j = 0; j < 54; ++j) s += g[i][j];
    return s;
}
int main(void) { report(bench()); return 0; }
