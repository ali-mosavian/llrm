/* Reduced from qb-qrender's d_faces.c: floats live past a float compare
   whose branches share its source position. */
typedef struct { float x, y, z, radius; } Light;
typedef struct { float nx, ny, nz, dist; } Plane;
typedef struct { float s[4], t[4]; } Tex;

long deep( Light *l, Plane *pl, Tex far *tx, float tms, float tmt, float w, float h, short n, float *out )
{
    long hits = 0;
    float su = out[0], sv = out[1], a = out[2], b = out[3];
    short i;
    for ( i = 0; i < n; i++, pl++ ) {
        float d = l->x * pl->nx + l->y * pl->ny + l->z * pl->nz - pl->dist;
        if ( d < l->radius && d > -l->radius ) {
            float fx = l->x - pl->nx * d, fy = l->y - pl->ny * d, fz = l->z - pl->nz * d;
            float r = l->radius + 1.0f;
            float ls = fx * tx->s[0] + fy * tx->s[1] + fz * tx->s[2] + tx->s[3] - tms;
            float lt = fx * tx->t[0] + fy * tx->t[1] + fz * tx->t[2] + tx->t[3] - tmt;
            if ( ls >= -r && ls <= w + r && lt >= -r && lt <= h + r )
                hits++;
        }
        su = su * a + sv; sv = sv * b + su;
    }
    out[0] = su; out[1] = sv;
    return hits;
}
