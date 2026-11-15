/* A lane written at a constant position (`insertelement` at 5, 3, 2): no lowering, no compile. */
typedef unsigned char lanes_b __attribute__((vector_size(16)));
typedef unsigned short lanes_w __attribute__((vector_size(16)));
typedef unsigned lanes_d __attribute__((vector_size(16)));
void set_lanes(lanes_b *b, lanes_w *w, lanes_d *d, unsigned char x, unsigned short y, unsigned z)
{
    lanes_b vb = *b;
    vb[5] = x;
    *b = vb;
    lanes_w vw = *w;
    vw[3] = y;
    *w = vw;
    lanes_d vd = *d;
    vd[2] = z;
    *d = vd;
}
