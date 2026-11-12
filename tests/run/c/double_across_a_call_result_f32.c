// flags: -O2
// The twin of double_across_a_call_result with float for double: SDCC's double is 32 bits ("types double, long double not supported. Assuming float"), so this is the program its columns run.
// Float literals carry `f` as well: SDCC reads every `double`, constant or type, as a float.
// A float live across a call that returns a float, with a reload placed between the call and the pseudo-op that takes its result:
// the stackifier refused it, "a call's floating result arrives on a stack that is not empty" (RFP-5, the QB runtime's B$CIRC at -Os).
extern void report(long value);

static float turn(float angle, int *spoked)
{
    *spoked = angle < 0;
    return angle < 0 ? -angle : angle;
}

static float (*volatile through)(float, int *) = turn;
static volatile int first = 1, second = 1;
static volatile float a = -3.0f, b = -4.0f;

int main(void)
{
    int s1 = 0, s2 = 0;
    float from = 0, to = 6.0f;

    if (first)
        from = through(a, &s1);
    if (second)
        to = through(b, &s2);
    report((long)(from * 10 + to));
    report(s1 * 2 + s2);
    return 0;
}
