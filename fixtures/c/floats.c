/* Floats as qcport uses them: compares, doubles, literals, results in ST0. */
#include <math.h>

extern float ext_scale( float x );

double half( double d )
{
    return d * 0.5;
}

float pick( float a, float b )
{
    if ( a < b ) return b;
    return a;
}

short sign( double d )
{
    return d >= 0.0 ? 1 : -1;
}

float scaled( float x )
{
    return ext_scale( x ) + (float) half( x );
}

double mag( double x, double y )
{
    return sqrt( x * x + y * y ) + fabs( x );
}

float negated( float x )
{
    float y = -x;
    return y;
}
