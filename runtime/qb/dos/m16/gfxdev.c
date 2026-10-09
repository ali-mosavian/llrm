/* The EGA graphics screen (QB rt/llega.asm, rt/llegasup.asm).

   A write goes through the graphics controller's write mode 2, which spreads
   the low four bits of the byte written over the planes, under the bit mask and
   the function applied to what the latches hold of the byte read first. The
   controller rests in write mode 2, function 0 and bit mask FFh while a
   graphics mode is on; the operations that change the function or the mask put
   them back. */
#include "device.h"
#include "gfxdev.h"


enum {
    GC_PORT = 0x3CE,
    GC_DATA_ROTATE = 3,
    GC_READ_MAP = 4,
    GC_MODE = 5,
    GC_BIT_MASK = 8,
    WRITE_MODE_1 = 1,
    WRITE_MODE_2 = 2,
    FUNCTION_SHIFT = 3,
    PLANES = 4,
    MODE_320 = 0x0D,
    TEXT_MODE_BIT = 0x80,
    GET_MODE = 0x0F00,
    SET_PALETTE = 0x1000,
    GET_FONT = 0x1130,
    FONT_14 = 0x0200,
    FONT_8 = 0x0300,
    FONT_16 = 0x0600
};

/* Volatile: a read loads the latches, which the bits a mask leaves alone come
   from, so it must not be dropped. */
typedef volatile u8 QB_FAR Video;
static Video *const screen = (Video *)0xA0000000UL;
static unsigned pitch = 80;       /* bytes of a row of one plane */

static void controller(unsigned index, unsigned value)
{
    dev_outw(GC_PORT, index | value << 8);
}

static Video *byte_of(unsigned x, unsigned y)
{
    return screen + (unsigned long)y * pitch + (x >> 3);
}

int gd_set_mode(unsigned mode)
{
    Regs r;

    r.rax = mode;
    dev_int10(&r);
    r.rax = GET_MODE;
    dev_int10(&r);
    pitch = mode == MODE_320 ? 40 : 80;
    if ((r.rax & 0x7F) != (mode & 0x7F))
        return 0;
    controller(GC_MODE, WRITE_MODE_2);
    return 1;
}

void gd_palette(unsigned index, unsigned color)
{
    Regs r;

    r.rax = SET_PALETTE;
    r.rbx = color << 8 | index;
    dev_int10(&r);
}

/* One byte of pixels under `mask`: `color` in the planes. */
static void put(Video *at, unsigned mask, unsigned color)
{
    controller(GC_BIT_MASK, mask);
    (void)*at;                   /* loads the latches */
    *at = (u8)color;
}

void gd_plot(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    if (operation)
        controller(GC_DATA_ROTATE, operation << FUNCTION_SHIFT);
    put(byte_of(x, y), 0x80 >> (x & 7), color);
    controller(GC_BIT_MASK, 0xFF);
    if (operation)
        controller(GC_DATA_ROTATE, 0);
}

void gd_span(unsigned x, unsigned count, unsigned y, unsigned color,
             unsigned operation)
{
    unsigned last = x + count - 1;
    unsigned left = 0xFF >> (x & 7), right = 0xFF << (7 - (last & 7)) & 0xFF;
    Video *at = byte_of(x, y);
    unsigned between = (last >> 3) - (x >> 3);

    if (count == 0)
        return;
    if (operation)
        controller(GC_DATA_ROTATE, operation << FUNCTION_SHIFT);
    if (between == 0) {
        put(at, left & right, color);
    } else {
        put(at++, left, color);
        controller(GC_BIT_MASK, 0xFF);
        while (--between) {
            (void)*at;
            *at++ = (u8)color;
        }
        put(at, right, color);
    }
    controller(GC_BIT_MASK, 0xFF);
    if (operation)
        controller(GC_DATA_ROTATE, 0);
}

unsigned gd_read(unsigned x, unsigned y)
{
    Video *at = byte_of(x, y);
    unsigned bit = 0x80 >> (x & 7), plane, color = 0;

    for (plane = PLANES; plane--;) {
        controller(GC_READ_MAP, plane);
        color = color << 1 | ((*at & bit) != 0);
    }
    return color;
}

void gd_move_rows(unsigned to, unsigned from, unsigned count)
{
    BlockOp op;

    op.dst = byte_of(0, to);
    op.src = byte_of(0, from);
    op.count = count * pitch;
    op.value = 1;                /* by bytes: a latch holds one */
    controller(GC_MODE, WRITE_MODE_1);
    dev_move(&op);
    controller(GC_MODE, WRITE_MODE_2);
}

void gd_glyph(unsigned x, unsigned y, const u8 QB_FAR *bits,
              unsigned height, unsigned foreground)
{
    Video *at = byte_of(x, y);

    while (height--) {
        put(at, 0xFF, 0);
        put(at, *bits++, foreground);
        at += pitch;
    }
    controller(GC_BIT_MASK, 0xFF);
}

const u8 QB_FAR *gd_font(unsigned height)
{
    Regs r;

    r.rax = GET_FONT;
    r.rbx = height == 14 ? FONT_14 : height == 16 ? FONT_16 : FONT_8;
    dev_int10(&r);
    return (const u8 QB_FAR *)((unsigned long)r.res << 16 | r.rbp);
}
