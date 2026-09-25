/* Inline assembly as qcport writes it: locals by name, and a result left in DX:AX. */

short chop( float f )
{
    short cw, result;
    __asm {
        fld    f
        fnstcw word ptr cw
        fistp  word ptr result
        fldcw  word ptr cw
    }
    return result;
}

unsigned long near raw( void )
{
    __asm mov ax, 1
    __emit__( 0x90 );
    __asm mov dx, 2
}

unsigned long twice( void )
{
    return raw() + raw();
}
