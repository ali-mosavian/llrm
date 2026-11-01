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
   fractional exponent are 5; 6 is a result past the extended range, y * log2|x|
   at 32767.5 (a result past a DOUBLE's is infinity, not an error). */
double *B_LPOW(double x, double y)
{
    if ((x == 0 && y < 0) || (x < 0 && fmod(y, 1) != 0))
        qb_error(5);
    if (x != 0 && y * log2(x < 0 ? -x : x) >= 32767.5)
        qb_error(6);
    power = pow(x, y);
    return &power;
}
