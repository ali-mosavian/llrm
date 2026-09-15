/* d_alias's vertex loop: fields and elements through near pointer arguments. */
typedef struct { float x, y, z; } Vec3;

static float vw[64];

void transform( short n, Vec3 *scale, float *m )
{
    short v;
    for ( v = 0; v < n; v++ )
        vw[v] = scale->y * m[3] + scale->z * m[7] + m[15];
}
