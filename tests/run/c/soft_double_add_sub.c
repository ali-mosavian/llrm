// flags: -O1 | -O2 | -Os
// bar: not comparable (SDCC double is 32 bits); twin soft_double_add_sub_f32
/* A double add and subtract through the soft-float library. At the 6502 the add read the flags of an unrelated copy
   (`ldx`) where the `and` of its select had set them: 1.5 + 0.25 came out as 0.5 (2026-11-11). */
extern void report(long value);
static volatile double a = 1.5, b = 0.25, c = 3.0, d = 0.125;
int main(void)
{
    report((long)((a + b) * 100.0));
    report((long)((c + d) * 1000.0));
    report((long)((a - b) * 100.0));
    return 0;
}
