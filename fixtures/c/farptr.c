/* snd_mix_statics' shape: a far pointer stored, reloaded and tested. */
typedef struct { short x, y; } Pt;

static Pt far *pts;
Pt far *other;
extern void far Use( Pt far *p );

short copy( void )
{
    pts = other;
    Use( pts );
    return pts ? 1 : 0;
}
