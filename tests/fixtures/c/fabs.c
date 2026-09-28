/* Reduced from qcport's mdl_ai.c: bcc -O's builtin fabs. */
#include <math.h>

double halfabs( double x )
{
    return fabs( x ) * 0.5;
}
