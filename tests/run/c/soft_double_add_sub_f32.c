// flags: -O1 | -O2 | -Os
// The twin of soft_double_add_sub with float for double: SDCC's double is 32 bits ("types double, long double not supported. Assuming float"), so this is the program its columns run.
// Float literals carry `f` as well: SDCC reads every `double`, constant or type, as a float.
/* A float add and subtract through the soft-float library. At the 6502 the add read the flags of an unrelated copy
   (`ldx`) where the `and` of its select had set them: 1.5 + 0.25 came out as 0.5 (2026-11-11). */
extern void report(long value);
static volatile float a = 1.5f, b = 0.25f, c = 3.0f, d = 0.125f;
int main(void)
{
    report((long)((a + b) * 100.0f));
    report((long)((c + d) * 1000.0f));
    report((long)((a - b) * 100.0f));
    return 0;
}
