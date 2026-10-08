// flags: -O0 | -O2 | -Os
// `a && b` and `a || b` as values, which a flat unit made a one-word place for (gcc.c-torture 20010129-1: "place n13 has
// incomplete extent"): the place is as wide as the value it holds.
extern void report(long value);

static int both(int a, int b) { return a && b; }
static int either(int a, int b) { return a || b; }

int main(void)
{
    int a, b, pattern = 0;
    long sum = 0;
    for (a = 0; a < 3; ++a)
        for (b = 0; b < 3; ++b) {
            int x = a && b;
            int y = a || b;
            pattern = pattern * 3 + x + y * 2 + both(a, b) * 5 + either(a, b) * 7;
            sum += (a && !b) + (b || (a && 0)) * 2;
        }
    report(pattern);
    report(sum);
    return 0;
}
