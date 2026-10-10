/* The graphics screen: each kind of mode has its table of operations, the planar
   EGA and VGA modes (QB rt/llega.asm, rt/llegasup.asm) and the linear 256-colour
   one.

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
    GC_COLOR_COMPARE = 2,
    GC_MODE = 5,
    GC_DONT_CARE = 7,
    GC_BIT_MASK = 8,
    WRITE_MODE_1 = 1,
    READ_MODE_1 = 8,
    WRITE_MODE_2 = 2,
    FUNCTION_SHIFT = 3,
    PLANES = 4,
    MODE_320 = 0x0D,
    MODE_LINEAR = 0x13,
    SET_COLOR_SELECT = 0x0B00,
    MODE_CGA4 = 4,
    MODE_CGA2 = 6,
    LAST_TEXT_MODE = 7,
    GET_MODE = 0x0F00,
    SET_PALETTE = 0x1000,
    SET_DAC = 0x1010,
    GET_FONT = 0x1130,
    FONT_14 = 0x0200,
    FONT_8 = 0x0300,
    FONT_16 = 0x0600
};

/* Volatile: a read loads the latches, which the bits a mask leaves alone come
   from, so it must not be dropped. */
typedef volatile u8 QB_FAR Video;
static Video *screen = (Video *)QB_VIDEO_MEMORY(0xA000);
static unsigned pitch = 80;       /* bytes of a row of one plane */

static void controller(unsigned index, unsigned value)
{
    dev_outw(GC_PORT, index | value << 8);
}

static Video *byte_of(unsigned x, unsigned y)
{
    return screen + (unsigned long)y * pitch + (x >> 3);
}

typedef struct GdOps {
    void (*plot)(unsigned x, unsigned y, unsigned color, unsigned operation);
    void (*span)(unsigned x, unsigned count, unsigned y, unsigned color, unsigned operation);
    unsigned (*read)(unsigned x, unsigned y);
    int (*search)(int x, int last, unsigned y, unsigned c1, unsigned c2, int match);
    void (*move_rows)(unsigned to, unsigned from, unsigned count);
    void (*glyph)(unsigned x, unsigned y, const u8 QB_FAR *bits, unsigned height, unsigned foreground);
} GdOps;

static const GdOps planar_ops, linear_ops, packed4_ops, packed2_ops;
static const GdOps *ops = &planar_ops;

int gd_set_mode(unsigned mode)
{
    Regs r;

    r.rax = mode;
    dev_int10(&r);
    r.rax = GET_MODE;
    dev_int10(&r);
    ops = mode == MODE_LINEAR ? &linear_ops : mode == MODE_CGA4 ? &packed4_ops : mode == MODE_CGA2 ? &packed2_ops : &planar_ops;
    pitch = mode == MODE_320 ? 40 : mode == MODE_LINEAR ? 320 : 80;
    if (mode == MODE_CGA4 || mode == MODE_CGA2)
        screen = (Video *)QB_VIDEO_MEMORY(0xB800);
    else
        screen = (Video *)QB_VIDEO_MEMORY(0xA000);
    if ((r.rax & 0x7F) != (mode & 0x7F))
        return 0;

    if (mode > LAST_TEXT_MODE && mode != MODE_LINEAR && mode != MODE_CGA4 && mode != MODE_CGA2)
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

void gd_cga_color(unsigned background, unsigned palette)
{
    Regs r;

    r.rax = SET_COLOR_SELECT;
    r.rbx = background;
    dev_int10(&r);
    r.rax = SET_COLOR_SELECT;
    r.rbx = 0x0100 | palette;
    dev_int10(&r);
}

void gd_palette_mix(unsigned index, unsigned red, unsigned green, unsigned blue)
{
    Regs r;

    r.rax = SET_DAC;
    r.rbx = index;
    r.rcx = green << 8 | blue;
    r.rdx = red << 8;
    dev_int10(&r);
}

/* One byte of pixels under `mask`: `color` in the planes. */
static void put(Video *at, unsigned mask, unsigned color)
{
    controller(GC_BIT_MASK, mask);
    (void)*at;                   /* loads the latches */
    *at = (u8)color;
}

static void planar_plot(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    if (operation)
        controller(GC_DATA_ROTATE, operation << FUNCTION_SHIFT);
    put(byte_of(x, y), 0x80 >> (x & 7), color);
    controller(GC_BIT_MASK, 0xFF);
    if (operation)
        controller(GC_DATA_ROTATE, 0);
}

static void planar_span(unsigned x, unsigned count, unsigned y, unsigned color,
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

static unsigned planar_read(unsigned x, unsigned y)
{
    Video *at = byte_of(x, y);
    unsigned bit = 0x80 >> (x & 7), plane, color = 0;

    for (plane = PLANES; plane--;) {
        controller(GC_READ_MAP, plane);
        color = color << 1 | ((*at & bit) != 0);
    }
    return color;
}

/* The pixels of the byte at `at` that are of colour `color`, a bit each, by the
   graphics controller's colour compare: one read for eight pixels. */
static unsigned equal_to(Video *at, unsigned color)
{
    controller(GC_COLOR_COMPARE, color);
    return *at;
}

static int planar_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match)
{
    int step = last >= x ? 1 : -1;

    controller(GC_DONT_CARE, 0x0F);
    controller(GC_MODE, READ_MODE_1 | WRITE_MODE_2);
    for (;;) {
        int base = x & ~7;
        int last_here = (last & ~7) == base;
        unsigned in = step > 0 ? 0xFF >> (x & 7) : 0xFF << (7 - (x & 7)) & 0xFF;
        Video *at = byte_of(x, y);
        unsigned found = equal_to(at, c1);

        if (c2 != c1)
            found |= equal_to(at, c2);
        if (!match)
            found = ~found;
        if (last_here)
            in &= step > 0 ? 0xFF << (7 - (last & 7)) & 0xFF : 0xFF >> (last & 7);
        found &= in;
        if (found) {
            int bit = 0;

            if (step > 0) {
                while (!(found << bit & 0x80))
                    bit++;
            } else {
                while (!(found >> bit & 1))
                    bit++;
                bit = 7 - bit;
            }
            controller(GC_MODE, WRITE_MODE_2);
            return base + bit;
        }
        if (last_here)
            break;
        x = step > 0 ? base + 8 : base - 1;
    }
    controller(GC_MODE, WRITE_MODE_2);
    return -1;
}

static void planar_move_rows(unsigned to, unsigned from, unsigned count)
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

static void planar_glyph(unsigned x, unsigned y, const u8 QB_FAR *bits,
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
    dev_int10r(&r);
    return (const u8 QB_FAR *)QB_REAL_POINTER(r.res, r.rbp);
}

/* The 256-colour mode: a byte a pixel, rows of 320. */
static Video *pixel_at(unsigned x, unsigned y)
{
    return screen + (unsigned long)y * pitch + x;
}

static unsigned combined(unsigned old, unsigned color, unsigned operation)
{
    switch (operation) {
    case 1:
        return old & color;
    case 2:
        return old | color;
    case 3:
        return old ^ color;
    default:
        return color;
    }
}

static void linear_plot(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    Video *at = pixel_at(x, y);

    *at = (u8)combined(*at, color, operation);
}

static void linear_span(unsigned x, unsigned count, unsigned y, unsigned color, unsigned operation)
{
    Video *at = pixel_at(x, y);

    while (count--) {
        *at = (u8)combined(*at, color, operation);
        at++;
    }
}

static unsigned linear_read(unsigned x, unsigned y)
{
    return *pixel_at(x, y);
}

static int linear_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match)
{
    int step = last >= x ? 1 : -1;
    Video *at = pixel_at(x, y);

    for (;; x += step, at += step) {
        int is = *at == c1 || *at == c2;

        if (is == match)
            return x;
        if (x == last)
            return -1;
    }
}

static void linear_move_rows(unsigned to, unsigned from, unsigned count)
{
    BlockOp op;

    op.dst = pixel_at(0, to);
    op.src = pixel_at(0, from);
    op.count = count * pitch;
    op.value = 0;
    dev_move(&op);
}

static void linear_glyph(unsigned x, unsigned y, const u8 QB_FAR *bits, unsigned height, unsigned foreground)
{
    while (height--) {
        Video *at = pixel_at(x, y++);
        unsigned row = *bits++, bit;

        for (bit = 0x80; bit; bit >>= 1)
            *at++ = row & bit ? (u8)foreground : 0;
    }
}

static const GdOps linear_ops = {
    linear_plot, linear_span, linear_read, linear_search, linear_move_rows, linear_glyph
};

static const GdOps planar_ops = {
    planar_plot, planar_span, planar_read, planar_search, planar_move_rows, planar_glyph
};

void gd_plot(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    ops->plot(x, y, color, operation);
}

void gd_span(unsigned x, unsigned count, unsigned y, unsigned color, unsigned operation)
{
    ops->span(x, count, y, color, operation);
}

unsigned gd_read(unsigned x, unsigned y)
{
    return ops->read(x, y);
}

int gd_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match)
{
    return ops->search(x, last, y, c1, c2, match);
}

void gd_move_rows(unsigned to, unsigned from, unsigned count)
{
    ops->move_rows(to, from, count);
}

void gd_glyph(unsigned x, unsigned y, const u8 QB_FAR *bits, unsigned height, unsigned foreground)
{
    ops->glyph(x, y, bits, height, foreground);
}

/* The CGA modes: pixels packed in bytes, `bits` each with the leftmost in the high
   bits, the even rows of the screen in the first 8 KB and the odd rows in the
   next, 80 bytes a row. */
enum { CGA_ODD_ROWS = 0x2000, CGA_ROW_BYTES = 80 };

static Video *packed_row(unsigned y)
{
    return screen + (y & 1 ? CGA_ODD_ROWS : 0) + (unsigned long)(y >> 1) * CGA_ROW_BYTES;
}

/* The byte of pixel `x` in a row, and the shift that brings its pixel to the low
   bits, for pixels of `bits` bits. */
static unsigned packed_shift(unsigned x, unsigned bits)
{
    return 8 - bits - (x * bits & 7);
}

static void packed_plot(unsigned x, unsigned y, unsigned color, unsigned operation, unsigned bits)
{
    Video *at = packed_row(y) + (x * bits >> 3);
    unsigned shift = packed_shift(x, bits), mask = ((1u << bits) - 1) << shift;
    unsigned old = *at, new_bits = combined(old >> shift & (mask >> shift), color, operation) << shift & mask;

    *at = (u8)(old & ~mask | new_bits);
}

static unsigned packed_read(unsigned x, unsigned y, unsigned bits)
{
    return *(packed_row(y) + (x * bits >> 3)) >> packed_shift(x, bits) & ((1u << bits) - 1);
}

static void packed_span(unsigned x, unsigned count, unsigned y, unsigned color, unsigned operation, unsigned bits)
{
    while (count--)
        packed_plot(x++, y, color, operation, bits);
}

static int packed_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match, unsigned bits)
{
    int step = last >= x ? 1 : -1;

    for (;; x += step) {
        unsigned color = packed_read(x, y, bits);

        if ((color == c1 || color == c2) == match)
            return x;
        if (x == last)
            return -1;
    }
}

static void packed_move_rows(unsigned to, unsigned from, unsigned count)
{
    BlockOp op;

    if (to < from) {
        while (count--) {
            op.dst = packed_row(to++);
            op.src = packed_row(from++);
            op.count = CGA_ROW_BYTES;
            op.value = 0;
            dev_move(&op);
        }
    } else {
        to += count;
        from += count;
        while (count--) {
            op.dst = packed_row(--to);
            op.src = packed_row(--from);
            op.count = CGA_ROW_BYTES;
            op.value = 0;
            dev_move(&op);
        }
    }
}

static void packed_glyph(unsigned x, unsigned y, const u8 QB_FAR *bits, unsigned height, unsigned foreground, unsigned depth)
{
    while (height--) {
        unsigned row = *bits++, bit, at = x;

        for (bit = 0x80; bit; bit >>= 1)
            packed_plot(at++, y, row & bit ? foreground : 0, 0, depth);
        y++;
    }
}

static void packed4_plot(unsigned x, unsigned y, unsigned c, unsigned o) { packed_plot(x, y, c, o, 2); }
static void packed4_span(unsigned x, unsigned n, unsigned y, unsigned c, unsigned o) { packed_span(x, n, y, c, o, 2); }
static unsigned packed4_read(unsigned x, unsigned y) { return packed_read(x, y, 2); }
static int packed4_search(int x, int l, unsigned y, unsigned a, unsigned b, int m) { return packed_search(x, l, y, a, b, m, 2); }
static void packed4_glyph(unsigned x, unsigned y, const u8 QB_FAR *b, unsigned h, unsigned f) { packed_glyph(x, y, b, h, f, 2); }
static void packed2_plot(unsigned x, unsigned y, unsigned c, unsigned o) { packed_plot(x, y, c, o, 1); }
static void packed2_span(unsigned x, unsigned n, unsigned y, unsigned c, unsigned o) { packed_span(x, n, y, c, o, 1); }
static unsigned packed2_read(unsigned x, unsigned y) { return packed_read(x, y, 1); }
static int packed2_search(int x, int l, unsigned y, unsigned a, unsigned b, int m) { return packed_search(x, l, y, a, b, m, 1); }
static void packed2_glyph(unsigned x, unsigned y, const u8 QB_FAR *b, unsigned h, unsigned f) { packed_glyph(x, y, b, h, f, 1); }

static const GdOps packed4_ops = {
    packed4_plot, packed4_span, packed4_read, packed4_search, packed_move_rows, packed4_glyph
};

static const GdOps packed2_ops = {
    packed2_plot, packed2_span, packed2_read, packed2_search, packed_move_rows, packed2_glyph
};
