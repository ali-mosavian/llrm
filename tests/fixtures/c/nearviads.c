/* Reduced from qcport's snd_mix.c: snd_mix_paint, which dsp.asm's IRQ
   runs on its own stack with DS still DGROUP. */
typedef struct { long pos, end; short left, right; } Chan;
typedef struct { short master; int far *paint_l; int far *paint_r; Chan far *chan; } Mix;

long save_map( short slot );
void restore_map( long saved, short slot );
void chan_paint( Mix *mix, Chan far *ch, short n );

void paint( Mix *mix, unsigned char far *dst, short frames )
{
    int far *left = mix->paint_l;
    int far *right = mix->paint_r;
    long saved = save_map( 2 );
    short n, done = 0, k, c;
    int vl, vr;

    while ( done < frames ) {
        n = (short) ( frames - done );
        if ( n > 512 ) n = 512;
        for ( k = 0; k < n; k++ ) left[k] = right[k] = 0;
        for ( c = 0; c < 18; c++ )
            if ( mix->chan[c].end ) chan_paint( mix, &mix->chan[c], n );
        for ( k = 0; k < n; k++ ) {
            vl = left[k] >> 5; vr = right[k] >> 5;
            if ( vl > 127 ) vl = 127; else if ( vl < -128 ) vl = -128;
            if ( vr > 127 ) vr = 127; else if ( vr < -128 ) vr = -128;
            if ( mix->master != 255 ) {
                vl = (int) ( (long) vl * mix->master / 255L );
                vr = (int) ( (long) vr * mix->master / 255L );
            }
            dst[(done + k) * 2]     = (unsigned char) ( vl + 128 );
            dst[(done + k) * 2 + 1] = (unsigned char) ( vr + 128 );
        }
        done = (short) ( done + n );
    }
    restore_map( saved, 2 );
}
