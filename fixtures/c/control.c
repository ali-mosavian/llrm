/* The integer constructs qcport uses beyond pal, qglsurf and ls. */

short pick( short k )
{
    switch ( k ) {
    case 1:  return 10;
    case 2:
    case 3:
    case 4:  return 20;
    case 9:  return 90;
    default: return -1;
    }
}

long shifted( long v, short n, unsigned short u )
{
    return ( v << n ) + ( v >> n ) + ( u >> n );
}

long grow( long total, short step )
{
    total += step;
    return total;
}

unsigned long far_ticks( void )
{
    return *(unsigned long far *) 0x0040006CUL;
}

unsigned short per( unsigned short offset )
{
    return offset / 7 + offset % 7;
}

float cell;

void far *where( void )
{
    return (void far *) &cell;
}

short comma( short a )
{
    return ( a++, a + 1 );
}
