/* A spill store sunk into the one successor that reads its slot went past a later store to the same slot: the stride of pack_ref
   (planes * per_plane) was stored over by planes, and the rows of a packed box were planes bytes apart (qb-m32's fill_ops, -m32 -Os). */
extern void report(long value);
static unsigned long acc_;
#define report(v) (acc_ = acc_ * 31UL + (unsigned long)(v))


static unsigned seed = 7;

unsigned gd_read(unsigned x, unsigned y)
{
    seed = seed * 1103515245u + 12345u;
    return (seed >> 16 & 15) ^ (x * 3 + y);
}

void pack_ref(unsigned mode, unsigned bx, unsigned by, unsigned w, unsigned h, unsigned char *out)
{
    unsigned planes = mode == 0x10 || mode == 0x0D ? 4 : 1, bits = mode == 0x13 ? 8 : mode == 4 ? 2 : 1;
    unsigned per_plane = (w * bits + 7) / 8, x, y, i;

    for (y = 0; y < h; y++, out += planes * per_plane) {
        for (i = 0; i < planes * per_plane; i++)
            out[i] = 0;
        for (x = 0; x < w; x++) {
            unsigned color = gd_read(bx + x, by + y), plane, at = x * bits;

            if (planes > 1) {
                for (plane = 0; plane < planes; plane++)
                    if (color >> plane & 1)
                        out[plane * per_plane + (x >> 3)] |= 0x80 >> (x & 7);
            } else {
                out[at >> 3] |= (unsigned char)(color << (8 - bits - (at & 7)));
            }
        }
    }
}

static int run_main(void)
{
    static const unsigned modes[] = { 0x13, 4, 6, 0x10, 0x0D };
    unsigned char packed[400];
    unsigned m, i;
    unsigned sum;

    for (m = 0; m < 5; m++) {
        for (i = 0; i < sizeof packed; i++)
            packed[i] = 0xEE;
        pack_ref(modes[m], 20, 7, 19, 7, packed);
        for (sum = 0, i = 0; i < sizeof packed; i++)
            sum = sum * 31u + packed[i];
        report((long)(sum & 0x7FFFFFFFu));
    }
    return 0;
}

long bench_x_spill_store_order(int n)
{
    int k;
    acc_ = 0;
    for (k = 0; k < n; ++k) run_main();
    return (long)acc_;
}

#undef report
int main(void)
{
    report(bench_x_spill_store_order(1));
    return 0;
}
