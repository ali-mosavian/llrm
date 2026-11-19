// flags: -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv | -O1 -march=i486 -m32 | -O2 -march=i486
// bar: not comparable (SDCC double is 32 bits: it computes a different program)
/* Float and double arguments to a call, mixed with integers: each goes into the space made for it on the stack, the order the callee reads. */
extern void report(long value);

static double mix(double a, float b, int n, double c, float d)
{
    if (n == 0) return a + b + c + d;
    return mix(a * 0.5 + 1.0, b + 1.0f, n - 1, c - a, d * 2.0f);
}

static long scaled(double a, float b, int n, double c, float d)
{
    return (long)(mix(a, b, n, c, d) * 1000.0 + 0.5);
}

/* The arguments are read from memory: constants fold the whole recursion at -O2 and no call is made with them. */
static volatile double d100 = 100.0, dm85 = -8.5, d7 = 7.0, d2 = 2.0;
static volatile float f3 = 3.0f, f15 = 1.5f, f025 = 0.25f, f1 = 1.0f;
static volatile int n0 = 0, n5 = 5, n9 = 9;

int main(void)
{
    report(scaled(d100, f3, n0, d7, f025));
    report(scaled(d100, f3, n5, d7, f025));
    report(scaled(dm85, f15, n9, d2, f1));
    return 0;
}
