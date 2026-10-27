/* The graphics screen: each kind of mode has its table of operations, the
   them back. */
#include "device.h"
enum {
    GC_READ_MAP = 4,
    GC_MODE = 5,
    GC_BIT_MASK = 8,
    WRITE_MODE_2 = 2,
    PLANES = 4,
    MODE_LINEAR = 0x13,
    MODE_CGA4 = 4,
    MODE_CGA2 = 6,
};
typedef volatile u8 QB_FAR Video;
static Video *screen = (Video *)QB_VIDEO_MEMORY(0xA000);
static unsigned pitch = 80; /* bytes of a row of one plane */
typedef struct GdOps {
    unsigned (*read)(unsigned x, unsigned y);
} GdOps;
enum { KIND_PLANAR, KIND_LINEAR, KIND_PACKED4, KIND_PACKED2, KINDS };
static unsigned kind = KIND_PLANAR;
int gd_set_mode(unsigned mode)
{
    kind = mode == MODE_LINEAR ? KIND_LINEAR
                               : KIND_PLANAR;
}
static void planar_plot(unsigned x, unsigned y, unsigned color, unsigned operation)
{
}
static unsigned planar_read(unsigned x, unsigned y)
{
    unsigned bit = 0x80 >> (x & 7), plane, color = 0;
    for (plane = PLANES; plane--;) {
    }
}
static int planar_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match)
{
    unsigned first_byte = (unsigned)x >> 3, last_byte = (unsigned)last >> 3;
    FillScan scan;
    {
        unsigned bit = 0;
    }
}
static void planar_move_rows(unsigned to, unsigned from, unsigned count)
{
}
static void planar_glyph(unsigned x, unsigned y, const u8 QB_FAR *bits, unsigned height,
                         unsigned foreground)
{
    Video *at = byte_of(x, y);
    while (height--) {
    }
}
static Video *pixel_at(unsigned x, unsigned y)
{
}
static void linear_plot(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    Video *at = pixel_at(x, y);
}
static unsigned linear_read(unsigned x, unsigned y)
{
}
static int linear_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match)
{
}
static void linear_move_rows(unsigned to, unsigned from, unsigned count)
{
    BlockOp op;
}
/* The CGA modes: pixels packed in bytes, `bits` each with the leftmost in the
 * colour, and the run of non-`edge` pixels it is in. */
static int packed4_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match)
{
}
static int packed2_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match)
{
}
static void packed_move_rows(unsigned to, unsigned from, unsigned count)
{
    BlockOp op;
    if (to < from && !((to ^ from) & 1)) {
        /* a scroll by an even number of rows: the rows of each bank are one
         * block of the bank */
        unsigned bank, offset, n;
        for (bank = 0; bank < 2; bank++) {
        }
        while (count--) {
        }
    }
}
/* Eight points of a circle step at once, without a call for each (set only: the
   array byte, so each byte of one is made of two of the other. */
typedef volatile u32 QB_FAR Video32;
#define LINEAR_PUT(name, combine)                                                                  \
    static void name(Video *to, const u8 QB_FAR *from, unsigned count, u32 flip)                   \
    {                                                                                              \
        Video32 *d = (Video32 *)to;                                                                \
        unsigned n = count >> 2;                                                                   \
        Video *db;                                                                                 \
        while (n--) {                                                                              \
        }                                                                                          \
    }
LINEAR_PUT(linear_put_set, COMBINE_SET)
LINEAR_PUT(linear_put_and, COMBINE_AND)
LINEAR_PUT(linear_put_or, COMBINE_OR)
LINEAR_PUT(linear_put_xor, COMBINE_XOR)
void gd_get(unsigned x, unsigned y, unsigned width, unsigned rows, u8 QB_FAR *out)
{
    unsigned row;
    if (kind == KIND_LINEAR) {
        for (row = 0; row < rows; row++, out += width) {
        }
    } else {
        unsigned bits = kind == KIND_PLANAR    ? 1
                                               : 1,
                 planes = kind == KIND_PLANAR ? PLANES : 1, plane;
        FillXfer xfer;
        for (plane = 0; plane < planes; plane++) {
            if (kind == KIND_PLANAR) {
            }
        }
    }
}
void gd_put(unsigned x, unsigned y, unsigned width, unsigned rows, const u8 QB_FAR *in,
            unsigned operation, unsigned invert)
{
    unsigned row;
    if (kind == KIND_LINEAR) {
        static void (*const put_run[4])(Video *to, const u8 QB_FAR *from, unsigned count,
                                        u32 flip) = {linear_put_set, linear_put_and, linear_put_or,
                                                     linear_put_xor};
        for (row = 0; row < rows; row++, in += width) {
            if (operation == 0 && !invert) {
                BlockOp op;
                dev_move(&op);
            } else {
                put_run[operation](pixel_at(x, y + row), in, width, invert ? 0xFFFFFFFFUL : 0);
            }
        }
    } else {
        unsigned bits = kind == KIND_PACKED4 ? 2 : 1, planes = kind == KIND_PLANAR ? PLANES : 1,
                 plane;
        FillXfer xfer;
        for (plane = 0; plane < planes; plane++) {
        }
    }
}