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
