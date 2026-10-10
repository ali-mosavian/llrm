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
    int back = last < x;
    unsigned first_byte = (unsigned)x >> 3, last_byte = (unsigned)last >> 3;
    FillScan scan;

    scan.at = byte_of(x, y);
    scan.count = (back ? first_byte - last_byte : last_byte - first_byte) + 1;
    scan.c1 = c1;
    scan.c2 = c2;
    scan.flags = (match ? 1 : 0) | (back ? 2 : 0);
    scan.first = back ? 0xFF << (7 - (x & 7)) & 0xFF : 0xFF >> (x & 7);
    scan.last = back ? 0xFF >> (last & 7) : 0xFF << (7 - (last & 7)) & 0xFF;
    controller(GC_DONT_CARE, 0x0F);
    controller(GC_MODE, READ_MODE_1 | WRITE_MODE_2);
    dev_scan_planar(&scan);
    controller(GC_MODE, WRITE_MODE_2);
    if (scan.found < 0)
        return -1;
    {
        unsigned bit = 0;

        if (back) {
            while (!(scan.hits >> bit & 1))
                bit++;
            return (int)((first_byte - (unsigned)scan.found) * 8 + 7 - bit);
        }
        while (!(scan.hits << bit & 0x80))
            bit++;
        return (int)((first_byte + (unsigned)scan.found) * 8 + bit);
    }
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
    FillScan scan;

    scan.at = pixel_at(x, y);
    scan.count = (unsigned)(step > 0 ? last - x : x - last) + 1;
    scan.c1 = c1;
    scan.c2 = c2;
    scan.flags = (match ? 1 : 0) | (step < 0 ? 2 : 0);
    dev_scan_linear(&scan);
    return scan.found < 0 ? -1 : x + step * scan.found;
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

/* The CGA modes' scan, a byte of pixels at a time (B$FSCC): the masks of the pixels of the first and the last byte that are in range
   go to the kernel, which gives the first byte with a wanted pixel in range and which pixels of it, and the first of those, from
   the left going right and from the right going left, is the pixel. */
static int packed_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match, unsigned bits)
{
    int back = last < x;
    unsigned per = 8 / bits, all = bits == 2 ? 0x55 : 0xFF, mask = (1u << bits) - 1;
    unsigned first_byte = (unsigned)x * bits >> 3, last_byte = (unsigned)last * bits >> 3;
    unsigned k0 = (unsigned)x % per, kl = (unsigned)last % per, k, base;
    FillScan scan;

    scan.at = packed_row(y) + first_byte;
    scan.count = (back ? first_byte - last_byte : last_byte - first_byte) + 1;
    scan.c1 = (c1 & mask) * (0xFF / mask);
    scan.c2 = (c2 & mask) * (0xFF / mask);
    scan.flags = (match ? 1 : 0) | (back ? 2 : 0) | (bits == 2 ? 4 : 0);
    scan.first = back ? all & (0xFF << ((per - 1 - k0) * bits)) & 0xFF : all & (0xFF >> (k0 * bits));
    scan.last = back ? all & (0xFF >> (kl * bits)) : all & (0xFF << ((per - 1 - kl) * bits)) & 0xFF;
    scan.middle = all;
    dev_scan_packed(&scan);
    if (scan.found < 0)
        return -1;
    if (back) {
        base = (first_byte - (unsigned)scan.found) * per;
        for (k = per; k--;)
            if (scan.hits & 1u << ((per - 1 - k) * bits))
                return (int)(base + k);
    } else {
        base = (first_byte + (unsigned)scan.found) * per;
        for (k = 0; k < per; k++)
            if (scan.hits & 1u << ((per - 1 - k) * bits))
                return (int)(base + k);
    }
    return -1;
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

/* One pixel: what a box would write to it. */
static void linear_dot_set(const GdFill *fill, unsigned x, unsigned y)
{
    *pixel_at(x, y) = (u8)fill->color;
}

static void linear_dot(const GdFill *fill, unsigned x, unsigned y)
{
    Video *at = pixel_at(x, y);

    *at = (u8)(*at & fill->keep ^ fill->flip);
}

static void packed_dot(const GdFill *fill, unsigned x, unsigned y, unsigned bits)
{
    Video *at = packed_row(y) + (x * bits >> 3);
    unsigned mask = ((1u << bits) - 1) << (8 - bits - (x * bits & 7));

    *at = (u8)(*at & (fill->keep | ~mask) ^ (fill->flip & mask));
}

static void packed4_dot(const GdFill *fill, unsigned x, unsigned y)
{
    packed_dot(fill, x, y, 2);
}

static void packed2_dot(const GdFill *fill, unsigned x, unsigned y)
{
    packed_dot(fill, x, y, 1);
}

static void planar_dot(const GdFill *fill, unsigned x, unsigned y)
{
    Video *at = byte_of(x, y);

    controller(GC_BIT_MASK, 0x80 >> (x & 7));
    (void)*at;                   /* loads the latches */
    *at = (u8)fill->color;
}

/* Eight points of a circle step at once, without a call for each (set only: the dots' `color` and masks as they are). */
static void linear_octet(const GdFill *fill, unsigned cx, unsigned cy, int u, int v, int w, int z)
{
    Video *c = pixel_at(cx, cy), *a, *b;
    unsigned long vp = (unsigned long)v * pitch, zp = (unsigned long)z * pitch;
    u8 color = (u8)fill->color;

    a = c - vp;
    b = c + vp;
    a[-u] = color;
    a[u] = color;
    b[-u] = color;
    b[u] = color;
    a = c - zp;
    b = c + zp;
    a[-w] = color;
    a[w] = color;
    b[-w] = color;
    b[w] = color;
}

static void packed4_octet(const GdFill *fill, unsigned cx, unsigned cy, int u, int v, int w, int z)
{
    packed_dot(fill, cx + u, cy - v, 2);
    packed_dot(fill, cx + w, cy - z, 2);
    packed_dot(fill, cx - w, cy - z, 2);
    packed_dot(fill, cx - u, cy - v, 2);
    packed_dot(fill, cx - u, cy + v, 2);
    packed_dot(fill, cx - w, cy + z, 2);
    packed_dot(fill, cx + w, cy + z, 2);
    packed_dot(fill, cx + u, cy + v, 2);
}

static void packed2_octet(const GdFill *fill, unsigned cx, unsigned cy, int u, int v, int w, int z)
{
    packed_dot(fill, cx + u, cy - v, 1);
    packed_dot(fill, cx + w, cy - z, 1);
    packed_dot(fill, cx - w, cy - z, 1);
    packed_dot(fill, cx - u, cy - v, 1);
    packed_dot(fill, cx - u, cy + v, 1);
    packed_dot(fill, cx - w, cy + z, 1);
    packed_dot(fill, cx + w, cy + z, 1);
    packed_dot(fill, cx + u, cy + v, 1);
}

static void planar_octet(const GdFill *fill, unsigned cx, unsigned cy, int u, int v, int w, int z)
{
    planar_dot(fill, cx + u, cy - v);
    planar_dot(fill, cx + w, cy - z);
    planar_dot(fill, cx - w, cy - z);
    planar_dot(fill, cx - u, cy - v);
    planar_dot(fill, cx - u, cy + v);
    planar_dot(fill, cx - w, cy + z);
    planar_dot(fill, cx + w, cy + z);
    planar_dot(fill, cx + u, cy + v);
}

static void (*const octets[KINDS])(const GdFill *fill, unsigned cx, unsigned cy, int u, int v, int w, int z) = {
    planar_octet, linear_octet, packed4_octet, packed2_octet
};

static void (*const dots[KINDS][2])(const GdFill *fill, unsigned x, unsigned y) = {
    { planar_dot, planar_dot },
    { linear_dot, linear_dot_set },
    { packed4_dot, packed4_dot },
    { packed2_dot, packed2_dot }
};

/* Lines: the loops of fill.asm, one for each kind of mode, with Bresenham's terms here.  They draw in the colour, as set. */
static void line_terms(FillLine *line, const GdFill *fill, unsigned dx, unsigned dy)
{
    int major = (int)(dx > dy ? dx : dy), minor = (int)(dx > dy ? dy : dx);

    line->count = (unsigned)major + 1;
    line->minor4 = 4 * minor;
    line->step4 = 4 * (minor - major);
    line->decision = 4 * minor - major;
    line->x_major = dx == 0 ? 2 : dx > dy;
    line->style = fill->style;
}

static void linear_line(const GdFill *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y)
{
    FillLine line;

    line_terms(&line, fill, dx, dy);
    line.dst = pixel_at(x, y);
    line.ystep = step_y > 0 ? (int)pitch : -(int)pitch;
    dev_line_linear(&line);
}

static void packed_line(const GdFill *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y, unsigned bits)
{
    FillLine line;

    line_terms(&line, fill, dx, dy);
    line.dst = packed_row(y) + (x * bits >> 3);
    line.pmask = ((1u << bits) - 1) << (8 - bits - (x * bits & 7));
    line.bpp = bits;
    line.color = fill->flip;
    line.ystep = step_y > 0 ? CGA_ROW_BYTES : -CGA_ROW_BYTES;
    dev_line_packed(&line);
}

static void packed4_line(const GdFill *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y)
{
    packed_line(fill, x, y, dx, dy, step_y, 2);
}

static void packed2_line(const GdFill *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y)
{
    packed_line(fill, x, y, dx, dy, step_y, 1);
}

static void planar_line(const GdFill *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y)
{
    FillLine line;

    line_terms(&line, fill, dx, dy);
    line.dst = byte_of(x, y);
    line.pmask = 0x80 >> (x & 7);
    line.ystep = step_y > 0 ? (int)pitch : -(int)pitch;
    line.color = fill->color;
    dev_line_planar(&line);
}

static void (*const lines[KINDS])(const GdFill *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y) = {
    planar_line, linear_line, packed4_line, packed2_line
};

void gd_dots_begin(const GdFill *fill)
{
    if (kind == KIND_PLANAR && fill->operation)
        controller(GC_DATA_ROTATE, fill->operation << FUNCTION_SHIFT);
}

void gd_dots_end(const GdFill *fill)
{
    if (kind == KIND_PLANAR) {
        controller(GC_BIT_MASK, 0xFF);
        if (fill->operation)
            controller(GC_DATA_ROTATE, 0);
    }
}

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
    fill->dot = dots[kind][operation == 0];
    fill->line = lines[kind];
    fill->octet = octets[kind];
    fill->color = color & 0xFF;
    fill->style = 0xFFFF;
    fill->operation = operation;
    fill->keep = operation == 0 ? 0 : operation == 1 ? byte : operation == 2 ? ~byte & 0xFF : 0xFF;
    fill->flip = operation == 1 ? 0 : byte;
    dev_fill_select(operation, byte);
}

/* GET and PUT.  A sub-byte plane (a CGA mode's pixels, an EGA plane's bits) is a run of bits that starts `shift` bits into
   a screen byte and at bit 0 of an array byte, so each byte of one is made of two of the other. */
typedef volatile u32 QB_FAR Video32;

#define COMBINE_SET(old, source) (source)
#define COMBINE_AND(old, source) ((old) & (source))
#define COMBINE_OR(old, source) ((old) | (source))
#define COMBINE_XOR(old, source) ((old) ^ (source))

#define LINEAR_PUT(name, combine) \
    static void name(Video *to, const u8 QB_FAR *from, unsigned count, u32 flip) \
    { \
        Video32 *d = (Video32 *)to; \
        const u32 QB_FAR *s = (const u32 QB_FAR *)from; \
        unsigned n = count >> 2; \
        Video *db; \
        const u8 QB_FAR *sb; \
        while (n--) { \
            u32 old_ = *d; \
            *d++ = combine(old_, *s++ ^ flip); \
        } \
        db = (Video *)d; \
        sb = (const u8 QB_FAR *)s; \
        n = count & 3; \
        while (n--) { \
            unsigned old_ = *db; \
            *db++ = (u8)combine(old_, (unsigned)(*sb++ ^ flip) & 0xFF); \
        } \
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
            BlockOp op;

            op.dst = out;
            op.src = pixel_at(x, y + row);
            op.count = width;
            op.value = 0;
            dev_move(&op);
        }
    } else {
        /* a plane (or the CGA modes' one) at a time, a pass of rows with the bits shifted into place */
        unsigned bits = kind == KIND_PLANAR ? 1 : kind == KIND_PACKED4 ? 2 : 1, planes = kind == KIND_PLANAR ? PLANES : 1, plane;
        unsigned total = width * bits, bytes = (total + 7) >> 3, shift = x * bits & 7;
        FillXfer xfer;

        xfer.rows = rows;
        xfer.abytes = bytes;
        xfer.sbytes = (shift + total + 7) >> 3;
        xfer.stride = planes * bytes;
        xfer.shift = shift;
        xfer.last = 0xFF << (bytes * 8 - total) & 0xFF;
        xfer.bank = kind != KIND_PLANAR;
        xfer.step = (int)pitch;
        for (plane = 0; plane < planes; plane++) {
            if (kind == KIND_PLANAR) {
                controller(GC_READ_MAP, plane);
                xfer.screen = byte_of(x, y);
            } else {
                xfer.screen = packed_row(y) + (x * bits >> 3);
            }
            xfer.array = out + plane * bytes;
            dev_get_rows(&xfer);
        }
    }
}

void gd_put(unsigned x, unsigned y, unsigned width, unsigned rows, const u8 QB_FAR *in, unsigned operation, unsigned invert)
{
    unsigned row;

    if (kind == KIND_LINEAR) {
        static void (*const put_run[4])(Video *to, const u8 QB_FAR *from, unsigned count, u32 flip) = {
            linear_put_set, linear_put_and, linear_put_or, linear_put_xor
        };

        for (row = 0; row < rows; row++, in += width) {
            if (operation == 0 && !invert) {
                BlockOp op;

                op.dst = pixel_at(x, y + row);
                op.src = in;
                op.count = width;
                op.value = 0;
                dev_move(&op);
            } else {
                put_run[operation](pixel_at(x, y + row), in, width, invert ? 0xFFFFFFFFUL : 0);
            }
        }
    } else {
        /* a plane (or the CGA modes' one) at a time, a pass of rows with the bits shifted into place */
        unsigned bits = kind == KIND_PACKED4 ? 2 : 1, planes = kind == KIND_PLANAR ? PLANES : 1, plane;
        unsigned total = width * bits, bytes = (total + 7) >> 3, shift = x * bits & 7;
        FillXfer xfer;

        xfer.rows = rows;
        xfer.abytes = bytes;
        xfer.sbytes = (shift + total + 7) >> 3;
        xfer.stride = planes * bytes;
        xfer.shift = shift;
        xfer.first = 0xFF >> shift;
        xfer.last = 0xFF << (xfer.sbytes * 8 - shift - total) & 0xFF;
        xfer.bank = kind != KIND_PLANAR;
        xfer.step = (int)pitch;
        dev_put_select(invert ? 4 : operation);
        if (kind != KIND_PLANAR) {
            xfer.screen = packed_row(y) + (x * bits >> 3);
            xfer.array = (void QB_FAR *)in;
            dev_put_rows(&xfer);
            return;
        }
        controller(GC_MODE, 0);
        if (operation)
            controller(GC_DATA_ROTATE, operation << FUNCTION_SHIFT);
        xfer.screen = byte_of(x, y);
        for (plane = 0; plane < planes; plane++) {
            dev_outw(0x3C4, 2 | 1 << plane << 8);
            xfer.rows = rows;
            xfer.array = (void QB_FAR *)(in + plane * bytes);
            dev_put_planar(&xfer);
        }
        dev_outw(0x3C4, 2 | 0x0F << 8);
        if (operation)
            controller(GC_DATA_ROTATE, 0);
        controller(GC_MODE, WRITE_MODE_2);
    }
}
