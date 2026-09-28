/* Reduced from qcport's d_alias.c: a far pointer tested against null. */
typedef struct { short lf[9]; } Cache;

short put( Cache far *lc, short v )
{
    if ( lc ) {
        lc->lf[1] = v;
        return 1;
    }
    return 0;
}
