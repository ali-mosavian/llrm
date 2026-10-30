// flags: -O0 | -O2 | -Os
// A loop to the terminator becomes a call of `strlen` where the ABI links one (-m32 -mabi=sysv); the library's own
// `strlen` (defined here: the test links no C library) keeps its loop, as the pin skips a function named for the idiom.
extern void report(long value);

unsigned strlen(const char *s)
{
    unsigned n = 0;
    while (s[n]) ++n;
    return n;
}

static int count(const char *s)
{
    int i = 0;
    while (s[i]) ++i;
    return i;
}

static const char *end_of(const char *s)
{
    while (*s) ++s;
    return s;
}

int main(void)
{
    const char *text = "strlen idiom";
    report(count(text));
    report(end_of(text) - text);
    report(count(""));
    report((long)strlen(text));
    return 0;
}
