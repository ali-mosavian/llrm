// flags: -O1 | -O2 | -Os
// A signed word against 0 or 1, as a branch, a select and a bool, is one test of the pair (z80: zero, then sign, in the
// carry). The edges are 0, 1, -1, 0x7fff and 0x8000, where a sign-only or zero-only test goes wrong.
extern void report(long value);

static int taken;
__attribute__((noinline)) void yes(void) { taken = 7; }
__attribute__((noinline)) void no(void) { taken = 3; }

__attribute__((noinline)) int gt0(short x) { if (x > 0) yes(); else no(); return taken; }
__attribute__((noinline)) int le0(short x) { if (x <= 0) yes(); else no(); return taken; }
__attribute__((noinline)) int ge1(short x) { if (x >= 1) yes(); else no(); return taken; }
__attribute__((noinline)) int lt1(short x) { if (x < 1) yes(); else no(); return taken; }

__attribute__((noinline)) int sel_gt0(short x) { return x > 0 ? 7 : 3; }
__attribute__((noinline)) int sel_le0(short x) { return x <= 0 ? 7 : 3; }
__attribute__((noinline)) int bool_ge1(short x) { return x >= 1; }
__attribute__((noinline)) int bool_lt1(short x) { return x < 1; }

int main(void)
{
    static const short v[] = {0, 1, -1, 2, -2, 255, 256, -256, 0x7fff, -0x7fff - 1, 0x4000, -0x4000};
    for (unsigned i = 0; i < sizeof v / sizeof v[0]; i++)
    {
        report(gt0(v[i]) * 1000 + le0(v[i]) * 100 + ge1(v[i]) * 10 + lt1(v[i]));
        report(sel_gt0(v[i]) * 1000 + sel_le0(v[i]) * 100 + bool_ge1(v[i]) * 10 + bool_lt1(v[i]));
    }
    return 0;
}
