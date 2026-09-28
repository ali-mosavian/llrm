/* Reduced from qcport's savegame.c: a stack local passed as a far pointer. */
short fill( short far *p );

short caller( void )
{
    short head[4];
    return fill( head );
}
