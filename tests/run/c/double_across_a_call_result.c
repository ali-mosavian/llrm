// flags: -O2
// A double live across a call that returns a double, with a reload placed between the call and the pseudo-op that takes its result:
// the stackifier refused it, "a call's floating result arrives on a stack that is not empty" (RFP-5, the QB runtime's B$CIRC at -Os).
extern void report(long value);

static double turn(double angle, int *spoked)
{
    *spoked = angle < 0;
    return angle < 0 ? -angle : angle;
}

static double (*volatile through)(double, int *) = turn;
static volatile int first = 1, second = 1;
static volatile double a = -3.0, b = -4.0;

int main(void)
{
    int s1 = 0, s2 = 0;
    double from = 0, to = 6.0;

    if (first)
        from = through(a, &s1);
    if (second)
        to = through(b, &s2);
    report((long)(from * 10 + to));
    report(s1 * 2 + s2);
    return 0;
}
