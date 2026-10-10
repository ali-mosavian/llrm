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
    unsigned (*read)(unsigned x, unsigned y);
    int (*search)(int x, int last, unsigned y, unsigned c1, unsigned c2, int match);
    void (*move_rows)(unsigned to, unsigned from, unsigned count);
    void (*glyph)(unsigned x, unsigned y, const u8 QB_FAR *bits, unsigned height, unsigned foreground);
} GdOps;

enum { KIND_PLANAR, KIND_LINEAR, KIND_PACKED4, KIND_PACKED2, KINDS };

static const GdOps planar_ops, linear_ops, packed4_ops, packed2_ops;
static const GdOps *ops = &planar_ops;
static unsigned kind = KIND_PLANAR;

int gd_set_mode(unsigned mode)
{
    Regs r;

    r.rax = mode;
    dev_int10(&r);
    r.rax = GET_MODE;
    dev_int10(&r);
    kind = mode == MODE_LINEAR ? KIND_LINEAR : mode == MODE_CGA4 ? KIND_PACKED4 : mode == MODE_CGA2 ? KIND_PACKED2 : KIND_PLANAR;
    ops = kind == KIND_LINEAR ? &linear_ops : kind == KIND_PACKED4 ? &packed4_ops : kind == KIND_PACKED2 ? &packed2_ops : &planar_ops;
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
    linear_plot, linear_read, linear_search, linear_move_rows, linear_glyph
};

static const GdOps planar_ops = {
    planar_plot, planar_read, planar_search, planar_move_rows, planar_glyph
};

void gd_plot(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    ops->plot(x, y, color, operation);
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
static unsigned packed4_read(unsigned x, unsigned y) { return packed_read(x, y, 2); }
static int packed4_search(int x, int l, unsigned y, unsigned a, unsigned b, int m) { return packed_search(x, l, y, a, b, m, 2); }
static void packed4_glyph(unsigned x, unsigned y, const u8 QB_FAR *b, unsigned h, unsigned f) { packed_glyph(x, y, b, h, f, 2); }
static void packed2_plot(unsigned x, unsigned y, unsigned c, unsigned o) { packed_plot(x, y, c, o, 1); }
static unsigned packed2_read(unsigned x, unsigned y) { return packed_read(x, y, 1); }
static int packed2_search(int x, int l, unsigned y, unsigned a, unsigned b, int m) { return packed_search(x, l, y, a, b, m, 1); }
static void packed2_glyph(unsigned x, unsigned y, const u8 QB_FAR *b, unsigned h, unsigned f) { packed_glyph(x, y, b, h, f, 1); }

static const GdOps packed4_ops = {
    packed4_plot, packed4_read, packed4_search, packed_move_rows, packed4_glyph
};

static const GdOps packed2_ops = {
    packed2_plot, packed2_read, packed2_search, packed_move_rows, packed2_glyph
};

/* Box fills: one routine for each kind of mode, chosen once by gd_fill_select with the operation patched into the box
   loop (fill.asm), and run on a box that has already been clipped to the screen.  The geometry is worked out once for
   the box; each row is a straight run of `op mem, imm`, with the byte at each end of a row patched where the pixels in
   it are not all the box's.  An edge byte is old & keep ^ flip, which every operation is: set keeps nothing and flips
   to the pattern; and keeps the pattern; or keeps what the pattern lacks and flips to it; xor keeps all and flips. */
static unsigned edge_and(const GdFill *fill, unsigned mask)
{
    return (fill->keep | ~mask) & 0xFF;
}

static unsigned edge_xor(const GdFill *fill, unsigned mask)
{
    return fill->flip & mask;
}

/* The 256-colour mode: a row is `count` bytes, `pitch` apart. */
static void linear_box(const GdFill *fill, unsigned x, unsigned y, unsigned count, unsigned rows)
{
    FillBox box;

    box.dst = pixel_at(x, y);
    box.rows = rows;
    box.middle = count;
    box.steps[0] = box.steps[1] = pitch;
    box.phase = 0;
    box.edges = 0;
    dev_fill_box(&box);
}

/* The CGA modes: the bytes at the ends of a row keep the pixels the box does not cover.  Rows alternate between the two
   banks of the screen. */
static void packed_box(const GdFill *fill, unsigned x, unsigned y, unsigned count, unsigned rows, unsigned bits)
{
    unsigned last = x + count - 1;
    unsigned first_byte = x * bits >> 3, last_byte = last * bits >> 3;
    unsigned left = 0xFF >> (x * bits & 7), right = 0xFF << (8 - bits - (last * bits & 7)) & 0xFF;
    Video *row = packed_row(y);
    FillBox box;

    if (first_byte == last_byte) {
        unsigned keep = edge_and(fill, left & right), flip = edge_xor(fill, left & right);

        while (rows--) {
            Video *at = row + first_byte;

            *at = (u8)(*at & keep ^ flip);
            row += y++ & 1 ? CGA_ROW_BYTES - CGA_ODD_ROWS : CGA_ODD_ROWS;
        }
        return;
    }
    box.dst = row + first_byte;
    box.rows = rows;
    box.middle = last_byte - first_byte - 1;
    box.steps[0] = CGA_ODD_ROWS;
    box.steps[1] = CGA_ROW_BYTES - CGA_ODD_ROWS;
    box.phase = y & 1 ? sizeof(unsigned) : 0;
    box.edges = 1;
    box.left = edge_and(fill, left) | edge_xor(fill, left) << 8;
    box.right = edge_and(fill, right) | edge_xor(fill, right) << 8;
    dev_fill_box(&box);
}

static void packed4_box(const GdFill *fill, unsigned x, unsigned y, unsigned count, unsigned rows)
{
    packed_box(fill, x, y, count, rows, 2);
}

static void packed2_box(const GdFill *fill, unsigned x, unsigned y, unsigned count, unsigned rows)
{
    packed_box(fill, x, y, count, rows, 1);
}

/* The planar modes: the colour goes through the controller's write mode 2, the operation through its function, and the
   bytes at the ends of a row under a bit mask.  What a mask covers is written down a whole column, so the mask is
   written three times for a box.  A set has the run of the box loop; the other operations read each byte to load the
   latches (a dword read would load only the last one). */
static void planar_edge(Video *at, unsigned rows, unsigned color)
{
    while (rows--) {
        (void)*at;
        *at = (u8)color;
        at += pitch;
    }
}

static void planar_box(const GdFill *fill, unsigned x, unsigned y, unsigned count, unsigned rows)
{
    unsigned last = x + count - 1, color = fill->color, function = fill->operation;
    unsigned left = 0xFF >> (x & 7), right = 0xFF << (7 - (last & 7)) & 0xFF;
    unsigned between = (last >> 3) - (x >> 3);
    Video *at = byte_of(x, y);

    if (function)
        controller(GC_DATA_ROTATE, function << FUNCTION_SHIFT);
    if (between == 0) {
        controller(GC_BIT_MASK, left & right);
        planar_edge(at, rows, color);
    } else {
        controller(GC_BIT_MASK, left);
        planar_edge(at, rows, color);
        controller(GC_BIT_MASK, 0xFF);
        if (between > 1) {
            if (function) {
                unsigned n;

                for (n = 1; n < between; n++)
                    planar_edge(at + n, rows, color);
            } else {
                FillBox box;

                box.dst = at + 1;
                box.rows = rows;
                box.middle = between - 1;
                box.steps[0] = box.steps[1] = pitch;
                box.phase = 0;
                box.edges = 0;
                dev_fill_box(&box);
            }
        }
        controller(GC_BIT_MASK, right);
        planar_edge(at + between, rows, color);
    }
    controller(GC_BIT_MASK, 0xFF);
    if (function)
        controller(GC_DATA_ROTATE, 0);
}

static void (*const boxes[KINDS])(const GdFill *fill, unsigned x, unsigned y, unsigned count, unsigned rows) = {
    planar_box, linear_box, packed4_box, packed2_box
};

void gd_fill_select(GdFill *fill, unsigned color, unsigned operation)
{
    unsigned byte = color & 0xFF;

    switch (kind) {
    case KIND_PACKED4:
        byte = (color & 3) * 0x55;
        break;
    case KIND_PACKED2:
        byte = (color & 1) * 0xFF;
        break;
    case KIND_PLANAR:
        byte = color & 0x0F;
        break;
    }
    operation &= 3;
    fill->box = boxes[kind];
    fill->color = color & 0xFF;
    fill->operation = operation;
    fill->keep = operation == 0 ? 0 : operation == 1 ? byte : operation == 2 ? ~byte & 0xFF : 0xFF;
    fill->flip = operation == 1 ? 0 : byte;
    dev_fill_select(operation, byte);
}
