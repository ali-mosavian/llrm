/* B_CHKEXP, B_CHKEX2, B_CHKLOG: the range checks of EXP, ^ and LOG, at their edges. */
#include <math.h>
#include <stdio.h>

extern double B_CHKEXP(double x);
extern double B_CHKEX2(double x);
extern double B_CHKLOG(double x);

static unsigned raised;
static int failures;

void qb_error(unsigned n)
{
    raised = n;
}

static void check(const char *name, double (*chk)(double), double x, unsigned want)
{
    double back;

    raised = 0;
    back = chk(x);
    if (raised != want) {
        printf("%s(%.17g) raised %u, want %u\n", name, x, raised, want);
        failures++;
    } else if (!raised && !(back == x || (x != x && back != back))) {
        printf("%s(%.17g) gave %.17g, not the operand\n", name, x, back);
        failures++;
    }
}

int main(void)
{
    const double e = 22712.700238998008;

    check("exp", B_CHKEXP, 0, 0);
    check("exp", B_CHKEXP, -1e308, 0);
    check("exp", B_CHKEXP, nextafter(e, 0), 0);
    check("exp", B_CHKEXP, e, 6);
    check("exp", B_CHKEXP, nextafter(e, 1e9), 6);
    check("exp", B_CHKEXP, INFINITY, 6);
    check("exp", B_CHKEXP, NAN, 0);
    check("exp2", B_CHKEX2, 32767.49, 0);
    check("exp2", B_CHKEX2, 32767.5, 6);
    check("exp2", B_CHKEX2, -INFINITY, 0);
    check("exp2", B_CHKEX2, NAN, 0);
    check("log", B_CHKLOG, 5e-324, 0);
    check("log", B_CHKLOG, 1, 0);
    check("log", B_CHKLOG, INFINITY, 0);
    check("log", B_CHKLOG, 0, 5);
    check("log", B_CHKLOG, -0.0, 5);
    check("log", B_CHKLOG, -1, 5);
    check("log", B_CHKLOG, -INFINITY, 5);
    check("log", B_CHKLOG, NAN, 0);
    printf(failures ? "failed\n" : "ok\n");
    return failures != 0;
}
