// flags: -O1 | -O2 | -O3 | -Os
/* A marker loop reads a new value each pass and switches on it (jpeg_fixed's). SimplifyCFG folded the header's switch into
   the one before the latch, so a marker no case named (0xD0) went round the range tests for ever at -O1 and up. */
extern void report(long value);
extern void message(const char *text);

static const int markers[] = {0xD0, 0xD9};
static int at;
static int __attribute__((noinline)) next(void) { return markers[at++]; }
void message(const char *text) { report(text[0]); }

static int __attribute__((noinline)) scan(void)
{
    for (;;) {
        int marker = next();
        if (marker == 0xC4) {
            next();
        } else if (marker == 0xC0) {
            next();
            next();
        } else if (marker == 0xC2) {
            message("p");
            return 0;
        } else if (marker >= 0xC9 && marker <= 0xCF && marker != 0xCC) {
            message("a");
            return 0;
        } else if (marker >= 0xC1 && marker <= 0xCF && marker != 0xC4 && marker != 0xC8 && marker != 0xCC) {
            message("b");
            return 0;
        } else if (marker == 0xDD) {
            next();
        } else if (marker == 0xD9) {
            break;
        }
    }
    return 1;
}

int main(void)
{
    report(scan());
    report(at);
    return 0;
}
