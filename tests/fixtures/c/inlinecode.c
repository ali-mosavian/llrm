/* Reduced from qcport's d_poly.c and sys_time.c: inline code naming frame
   places, and __emit__ answering in dx:ax. */
double sine( double rad )
{
    double result;
    __asm {
        fld   rad
        fsin
        fstp  result
    }
    return result;
}

unsigned long ticks( void )
{
    __emit__( 0x0f, 0x31 );
}
