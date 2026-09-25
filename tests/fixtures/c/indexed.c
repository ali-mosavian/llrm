/* d_draw_faces's per-vertex loops over static arrays. */
static float vt_x[16], vt_w[16], px[16];
static short sx[16], sy[16];

void project( short cnt, float h )
{
    short j;
    float rw;
    for ( j = 0; j < cnt; j++ ) {
        rw = 1.0f / vt_w[j];
        px[j] = h + vt_x[j] * rw * h;
    }
}

void copy( short cnt )
{
    short j;
    for ( j = 0; j < cnt; j++ ) sy[j] = sx[j];
}

/* snd_fetch's decode loop: a store through a pointer argument. */
void fill( signed char *out, short n )
{
    short j;
    for ( j = 0; j < n; j++ ) out[j] = (signed char) sx[j];
}

/* A local whose address is taken is reachable through the pointer. */
short keep( short n )
{
    short j, t = 0;
    short *p = &t;
    for ( j = 0; j < n; j++ ) *p = (short) ( *p + j );
    return t;
}

/* sc.c's sc_lru_use: block lists behind far pointers, list heads inline,
   all through a far struct pointer. */
typedef struct {
    short far *bord, far *bprev, far *bnext;
    long  far *bstamp;
    long  clock;
    short lhead[7], ltail[7];
} Cache;

void lru_use( Cache far *sc, short b )
{
    short c, t;

    if ( b < 0 ) return;
    sc->bstamp[b] = ++sc->clock;
    c = sc->bord[b];
    if ( sc->bprev[b] >= 0 || sc->bnext[b] >= 0 || sc->lhead[c] == b ) return;
    t = sc->ltail[c];
    sc->bprev[b] = t;
    sc->bnext[b] = -1;
    if ( t >= 0 ) sc->bnext[t] = b; else sc->lhead[c] = b;
    sc->ltail[c] = b;
}

/* sc.c's sc_init: a counted loop over a far struct's array, after stores through it. */
typedef struct { long slot, grn; short desc[25]; long clock; } Classes;

void clear( Classes far *sc )
{
    short i;

    sc->slot = 0; sc->grn = 0;
    for ( i = 0; i < 25; i++ ) sc->desc[i] = 0;
    sc->clock = 0;
}
