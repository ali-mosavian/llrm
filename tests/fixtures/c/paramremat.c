/* Reduced from qcport's cvar_register: float parameters read after calls. */
void tick( void );

float summed( float a, short n )
{
    float s = 0.0f;
    short i;

    for ( i = 0; i < n; i++ ) {
        tick();
        s += a;
    }
    return s;
}
