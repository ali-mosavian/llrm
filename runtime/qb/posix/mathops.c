/* mathops.h on a host: the math library's. */
#include <math.h>

#include "mathops.h"

void qb_sincos(const double *angle, double *sine, double *cosine)
{
    *sine = sin(*angle);
    *cosine = cos(*angle);
}
