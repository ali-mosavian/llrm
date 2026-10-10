/* Graphics GET and PUT: a rectangle of the screen to an array and back (QB
   rt/getput.asm B$GGET, B$GPUT).  The array holds the width and the height as
   two words, then the rows, each as one run of bytes per plane, the leftmost
   pixel the high bit: 4 + rows * planes * ((width + 7) / 8) bytes, as QB's
   formula for the array's size has it. */
#include "gfx.h"
#include "ad.h"

enum { HEADER = 4 };
enum { PUT_OR, PUT_AND, PUT_PRESET, PUT_PSET, PUT_XOR };

typedef u8 QB_FAR *Bytes;

/* The bytes of an array's elements. */
static unsigned long array_bytes(const AD *ad)
{
    unsigned long count = 1;
    unsigned at;

    for (at = 0; at < ad->dims; at++)
        count *= ad->dm[at].count;
    return count * ad->elem;
}

/* The bytes of a row of `width` pixels in one plane. */
static unsigned row_bytes(unsigned width)
{
    return (width * gfx_current->bits + 7) / 8;
}

/* B$GGET: GET (x1, y1)-(x2, y2), array; the corners were given by B$N1xx and
   B$N2xx. */
void B_GGET(qb_data_ptr data, const AD *ad)
{
    int left = gfx_x1 < gfx_x2 ? gfx_x1 : gfx_x2;
    int top = gfx_y1 < gfx_y2 ? gfx_y1 : gfx_y2;
    unsigned width = (gfx_x1 < gfx_x2 ? gfx_x2 - gfx_x1 : gfx_x1 - gfx_x2) + 1;
    unsigned height = (gfx_y1 < gfx_y2 ? gfx_y2 - gfx_y1 : gfx_y1 - gfx_y2) + 1;
    unsigned per_plane = row_bytes(width), x, y, plane;
    unsigned planes = gfx_current->planes, bits = gfx_current->bits;
    Bytes out = (Bytes)data;

    if (HEADER + (unsigned long)height * planes * per_plane > array_bytes(ad))
        qb_error(BE_ILLFUN);
    ((u16 QB_FAR *)out)[0] = width * bits;
    ((u16 QB_FAR *)out)[1] = height;
    out += HEADER;
    for (y = 0; y < height; y++) {
        for (plane = 0; plane < planes; plane++) {
            for (x = 0; x < per_plane; x++)
                out[plane * per_plane + x] = 0;
        }
        for (x = 0; x < width; x++) {
            int color = gfx_pixel(left + (int)x, top + (int)y);

            if (color <= 0)
                continue;
            if (planes > 1) {
                for (plane = 0; plane < planes; plane++) {
                    if (color >> plane & 1)
                        out[plane * per_plane + (x >> 3)] |= 0x80 >> (x & 7);
                }
            } else {
                /* a pixel is `bits` wide in the row, the leftmost in the high bits */
                unsigned at = x * bits;

                out[at >> 3] |= color << (8 - bits - (at & 7));
            }
        }
        out += planes * per_plane;
    }
}

/* B$GPUT: PUT (x, y), array, how; the corner was given by B$N1xx. */
void B_GPUT(qb_data_ptr data, const AD *ad, short how)
{
    Bytes in = (Bytes)data;
    unsigned planes = gfx_current->planes, bits = gfx_current->bits;
    unsigned width = ((u16 QB_FAR *)in)[0] / bits, height = ((u16 QB_FAR *)in)[1];
    unsigned per_plane = row_bytes(width), x, y, plane;

    if (how < PUT_OR || how > PUT_XOR
        || HEADER + (unsigned long)height * planes * per_plane
               > array_bytes(ad))
        qb_error(BE_ILLFUN);
    in += HEADER;
    for (y = 0; y < height; y++) {
        for (x = 0; x < width; x++) {
            unsigned color = 0;
            byte operation = OP_SET;

            if (planes > 1) {
                for (plane = 0; plane < planes; plane++) {
                    if (in[plane * per_plane + (x >> 3)] & (0x80 >> (x & 7)))
                        color |= 1u << plane;
                }
            } else {
                unsigned at = x * bits;

                color = in[at >> 3] >> (8 - bits - (at & 7)) & ((1u << bits) - 1);
            }
            if (how == PUT_PRESET)
                color = ~color & (gfx_current->colors - 1);
            else if (how == PUT_OR)
                operation = OP_OR;
            else if (how == PUT_AND)
                operation = OP_AND;
            else if (how == PUT_XOR)
                operation = OP_XOR;
            gfx_plot(gfx_x1 + (int)x, gfx_y1 + (int)y, (byte)color, operation);
        }
        in += planes * per_plane;
    }
}
#pragma aux B_GGET "B$GGET"
#pragma aux B_GPUT "B$GPUT"
