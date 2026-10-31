/* hand strlower against k_strlower on fresh copies: the returned pointer and every byte written.
   Covers empty, no change, all bytes 1..255, and strings of 255, 256 and 300 chars
   (the hand loop's offset wraps and bumps the high pointer byte). */
char *__fastcall__ strlower(char *s);
char *k_strlower(char *s);

extern char mix[], mix2[];

static char a[320], b[320];
static char all[256], rep[320];

/* init[n] must be the terminator */
static int run(const char *init, unsigned n)
{
    unsigned i;
    for (i = 0; i <= n; ++i)
        a[i] = b[i] = init[i];
    if (strlower(a) != a || k_strlower(b) != b)
        return 1;
    for (i = 0; i <= n; ++i)
        if (a[i] != b[i])
            return 1;
    return 0;
}

int main(void)
{
    unsigned i;
    for (i = 0; i < 255; ++i)
        all[i] = (char)(i + 1);
    for (i = 0; i < 300; ++i)
        rep[i] = (char)(i % 3 == 0 ? 'A' + i % 26 : i % 3 == 1 ? 'a' + i % 26 : '0' + i % 10);
    if (run("", 0)) return 1;
    if (run("abc", 3)) return 2;
    if (run("ABC", 3)) return 3;
    if (run("@[`{AZaz", 8)) return 4;
    if (run(mix, 16)) return 5;
    if (run(mix2, 16)) return 6;
    if (run(all, 255)) return 7;
    rep[255] = 0;
    if (run(rep, 255)) return 8;
    rep[255] = 'q';
    rep[256] = 0;
    if (run(rep, 256)) return 9;
    rep[256] = 'q';
    rep[300] = 0;
    if (run(rep, 300)) return 10;
    return 0;
}
