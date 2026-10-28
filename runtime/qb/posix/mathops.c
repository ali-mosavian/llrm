/* mathops.h on a host: the math library's. */
#include <math.h>

#include "mathops.h"

void qb_sincos(const double *angle, double *sine, double *cosine)
{
    *sine = sin(*angle);
    *cosine = cos(*angle);
}

/* The range checks of EXP, ^ and LOG, which the target's FPU does with its operand in ST0 (dos/<mode>/fpu.asm): a
   host's take the double and give it back. A NaN passes, as it does there. */
#include "qb.h"

double B_CHKEXP(double x)
{
    if (x >= 22712.700238998008)
        qb_error(BE_OVERFLOW);
    return x;
}

double B_CHKEX2(double x)
{
    if (x >= 32767.5)
        qb_error(BE_OVERFLOW);
    return x;
}

double B_CHKLOG(double x)
{
    if (x <= 0)
        qb_error(BE_ILLFUN);
    return x;
}
