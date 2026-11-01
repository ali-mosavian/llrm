/* mathops.h on a host: the math library's. */
#include <float.h>
#include <math.h>

#include "mathops.h"
#include "qb.h"

void qb_sincos(const double *angle, double *sine, double *cosine)
{
    *sine = sin(*angle);
    *cosine = cos(*angle);
}

static double power;

/* B$LPOW: `x ^ y` on a host, which has no x87 operands to give B$POW4. The
   errors are B$POW4's: 0 to a negative power and a negative base with a
   fractional exponent are 5, an infinite result 6. */
double *B_LPOW(double x, double y)
{
    if ((x == 0 && y < 0) || (x < 0 && fmod(y, 1) != 0))
        qb_error(5);
    power = pow(x, y);
    if (power > DBL_MAX || power < -DBL_MAX)
        qb_error(6);
    return &power;
}
