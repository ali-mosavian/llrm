/* The graphics and text screens of a host: the device interface's gd_* and dev_text_* over one framebuffer of
   pixel values (video.h). The GET and PUT arrays have the layout the DOS adapters give them (a plane's run
   of bits per row for the 16-colour modes, packed pixels for CGA, a byte a pixel for mode 13), so a program
   that saves and loads them sees no difference. */
#include <stdio.h>
#include <pthread.h>
#include <string.h>

#include "dev.h"
#include "gfxdev.h"
#include "vgadata.h"
#include "video.h"

enum {
    KIND_TEXT,
    KIND_PLANAR,
    KIND_LINEAR,
    KIND_PACKED4,
    KIND_PACKED2,
    PLANES = 4,
    MODE_320 = 0x0D,
    MODE_640 = 0x0E,
    MODE_350 = 0x10,
    MODE_480 = 0x12,
    MODE_LINEAR = 0x13,
    FONT_ROWS_8 = 8,
    FONT_ROWS_14 = 14,
    PALETTE_REGISTERS = 16,
    MODE_BIT_KEEP_SCREEN = 0x80
};

_Atomic uint8_t host_dirty[HOST_MAX_HEIGHT];

/* A platform that shows the frame (sdl) lets the first one of a new mode reach the screen here. */
/* A weak no-op, not a weak undefined reference: Mach-O's linker rejects the latter. */
__attribute__((weak)) void host_video_mode_changed(void) {}

/* The pages SCREEN can name (mode 0Dh has eight): drawing goes to `pixels`, the page being drawn, and the presenter reads `shown`. */
enum { MAX_PAGES = 8 };
static uint8_t page_store[MAX_PAGES][HOST_MAX_WIDTH * HOST_MAX_HEIGHT];
static uint8_t *pixels = page_store[0];
static _Atomic(const uint8_t *) shown = page_store[0];
static unsigned page_count = 1;
static unsigned width = HOST_TEXT_COLUMNS * HOST_CELL_WIDTH, height = HOST_TEXT_ROWS * HOST_CELL_HEIGHT;
static unsigned mode = HOST_MODE_TEXT, kind = KIND_TEXT, serial;
static unsigned pixel_mask = 0x0F;

/* Guards the mode, palette and cursor state the SDL thread reads while the program changes it. */
static pthread_mutex_t state_lock = PTHREAD_MUTEX_INITIALIZER;

void host_state_lock(void)
{
    pthread_mutex_lock(&state_lock);
}

void host_state_unlock(void)
{
    pthread_mutex_unlock(&state_lock);
}

static uint16_t cells[HOST_TEXT_ROWS][HOST_TEXT_COLUMNS];
static unsigned cursor_row, cursor_column;
static int cursor_visible = 1;

/* The DAC (6 bits a channel), the attribute controller's palette registers, and the CGA colour select. */
static uint8_t dac[256][3];
static uint8_t attributes[PALETTE_REGISTERS];
static unsigned cga_background, cga_palette = 1;

/* The attribute registers a mode starts with: the colours 0 to 5 and 7 as they are, 6 brown, 8 to 15 bright. */
static const uint8_t default_attributes[PALETTE_REGISTERS] = {0, 1, 2, 3, 4, 5, 0x14, 7,
                                                              0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F};

static void touch(unsigned top, unsigned count)
{
    unsigned y;

    for (y = top; y < top + count && y < HOST_MAX_HEIGHT; y++)
        atomic_store_explicit(&host_dirty[y], 1, memory_order_release);
}

static void touch_all(void)
{
    touch(0, height);
}

static uint8_t *pixel_at(unsigned x, unsigned y)
{
    return pixels + (size_t)y * width + x;
}

unsigned host_width(void)
{
    return width;
}

unsigned host_height(void)
{
    return height;
}

unsigned host_mode(void)
{
    return mode;
}

const uint8_t *host_pixels(void)
{
    return atomic_load_explicit(&shown, memory_order_acquire);
}

unsigned host_mode_serial(void)
{
    return serial;
}

void host_cursor(unsigned *row, unsigned *column, int *visible)
{
    *row = cursor_row;
    *column = cursor_column;
    *visible = cursor_visible;
}

static uint8_t eight_bits(uint8_t six)
{
    return (uint8_t)(six << 2 | six >> 4);
}

void host_colors(uint8_t rgb[256][3])
{
    unsigned value, channel;
    uint8_t six[256][3];

    memset(six, 0, sizeof six);
    switch (kind) {
    case KIND_LINEAR:
        memcpy(six, dac, sizeof six);
        break;
    case KIND_PACKED4:
    case KIND_PACKED2: {
        /* the colour select: the background, and the palette of 3 colours (green red brown, cyan magenta white) */
        static const uint8_t palettes[2][3] = {{2, 4, 6}, {3, 5, 7}};
        unsigned at;

        memcpy(six[0], dac[default_attributes[kind == KIND_PACKED4 ? cga_background : 0]], 3);
        if (kind == KIND_PACKED2) {
            memcpy(six[1], dac[default_attributes[15]], 3);
            break;
        }
        for (at = 0; at < 3; at++)
            memcpy(six[1 + at], dac[default_attributes[palettes[cga_palette][at]]], 3);
        break;
    }
    default:
        for (value = 0; value < PALETTE_REGISTERS; value++)
            memcpy(six[value], dac[attributes[value] & 0xFF], 3);
    }
    for (value = 0; value < 256; value++)
        for (channel = 0; channel < 3; channel++)
            rgb[value][channel] = eight_bits(six[value][channel]);
}

/* The state a mode starts with. */
static void reset_palette(void)
{
    memcpy(dac, qb_vga_palette, sizeof dac);
    if (kind != KIND_LINEAR)
        memcpy(dac, qb_ega_palette, sizeof qb_ega_palette);
    memcpy(attributes, default_attributes, sizeof attributes);
    cga_background = 0;
    cga_palette = 1;
}

static void draw_cell(unsigned row, unsigned column)
{
    unsigned value = cells[row][column], character = value & 0xFF, foreground = value >> 8 & 0x0F,
             background = value >> 12 & 0x07, line, bit;
    const uint8_t *glyph = qb_vga_font_16 + character * HOST_CELL_HEIGHT;

    for (line = 0; line < HOST_CELL_HEIGHT; line++) {
        uint8_t *at = pixel_at(column * HOST_CELL_WIDTH, row * HOST_CELL_HEIGHT + line);

        for (bit = 0; bit < HOST_CELL_WIDTH; bit++)
            at[bit] = (uint8_t)(glyph[line] & 0x80 >> bit ? foreground : background);
    }
    touch(row * HOST_CELL_HEIGHT, HOST_CELL_HEIGHT);
}

void host_video_reset(void)
{
    gd_set_mode(HOST_MODE_TEXT);
}

int gd_set_mode(unsigned requested)
{
    unsigned number = requested & ~(unsigned)MODE_BIT_KEEP_SCREEN;
    unsigned new_kind, new_width, new_height, row, column;

    switch (number) {
    case HOST_MODE_TEXT:
        new_kind = KIND_TEXT, new_width = HOST_TEXT_COLUMNS * HOST_CELL_WIDTH,
        new_height = HOST_TEXT_ROWS * HOST_CELL_HEIGHT;
        break;
    case HOST_MODE_CGA4:
        new_kind = KIND_PACKED4, new_width = 320, new_height = 200;
        break;
    case HOST_MODE_CGA2:
        new_kind = KIND_PACKED2, new_width = 640, new_height = 200;
        break;
    case MODE_320:
        new_kind = KIND_PLANAR, new_width = 320, new_height = 200;
        break;
    case MODE_640:
        new_kind = KIND_PLANAR, new_width = 640, new_height = 200;
        break;
    case MODE_350:
        new_kind = KIND_PLANAR, new_width = 640, new_height = 350;
        break;
    case MODE_480:
        new_kind = KIND_PLANAR, new_width = 640, new_height = 480;
        break;
    case MODE_LINEAR:
        new_kind = KIND_LINEAR, new_width = 320, new_height = 200;
        break;
    default:
        return 0;
    }
    host_state_lock();
    mode = number;
    kind = new_kind;
    width = new_width;
    height = new_height;
    pixel_mask = kind == KIND_LINEAR ? 0xFF : kind == KIND_PACKED4 ? 3 : kind == KIND_PACKED2 ? 1 : 0x0F;
    reset_palette();
    memset(page_store, 0, sizeof page_store);
    page_count = number == MODE_320 ? 8 : number == MODE_640 ? 4 : number == MODE_350 ? 2 : 1;
    pixels = page_store[0];
    atomic_store_explicit(&shown, page_store[0], memory_order_release);
    if (kind == KIND_TEXT) {
        for (row = 0; row < HOST_TEXT_ROWS; row++)
            for (column = 0; column < HOST_TEXT_COLUMNS; column++)
                cells[row][column] = 0x0700 | ' ';
        cursor_row = cursor_column = 0;
        cursor_visible = 1;
    }
    serial++;
    touch_all();
    host_state_unlock();
    host_video_mode_changed();
    return 1;
}

int gd_set_pages(unsigned active, unsigned visible)
{
    if (active >= page_count || visible >= page_count)
        return 0;
    host_state_lock();
    pixels = page_store[active];
    atomic_store_explicit(&shown, page_store[visible], memory_order_release);
    touch_all();
    host_state_unlock();
    return 1;
}

void gd_palette(unsigned index, unsigned color)
{
    host_state_lock();
    if (kind == KIND_PLANAR || kind == KIND_TEXT) {
        attributes[index % PALETTE_REGISTERS] = (uint8_t)(color & 0x3F);
        touch_all();
    }
    host_state_unlock();
}

void gd_cga_color(unsigned background, unsigned palette)
{
    host_state_lock();
    cga_background = background & 15;
    cga_palette = palette & 1;
    touch_all();
    host_state_unlock();
}

void gd_palette_mix(unsigned index, unsigned red, unsigned green, unsigned blue)
{
    uint8_t *entry = dac[index & 0xFF];

    host_state_lock();
    entry[0] = (uint8_t)(red & 0x3F);
    entry[1] = (uint8_t)(green & 0x3F);
    entry[2] = (uint8_t)(blue & 0x3F);
    touch_all();
    host_state_unlock();
}

static unsigned combined(unsigned old, unsigned color, unsigned operation)
{
    switch (operation & 3) {
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

static void blend(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    uint8_t *at = pixel_at(x, y);

    *at = (uint8_t)(combined(*at, color & pixel_mask, operation) & pixel_mask);
}

void gd_plot(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    if (x < width && y < height) {
        blend(x, y, color, operation);
        touch(y, 1);
    }
}

unsigned gd_read(unsigned x, unsigned y)
{
    return x < width && y < height ? *pixel_at(x, y) : 0;
}

int gd_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match)
{
    int step = last >= x ? 1 : -1, at;

    for (at = x;; at += step) {
        unsigned value = *pixel_at((unsigned)at, y);
        int wanted = value == c1 || value == c2;

        if (wanted == (match != 0))
            return at;
        if (at == last)
            return -1;
    }
}

int gd_run(int x, int last, unsigned y, unsigned edge, unsigned paint, int limit, int *left, int *right)
{
    int open = gd_search(x, last, y, edge, paint, 0), l, r;

    if (open < 0)
        return 0;
    l = gd_search(open, 0, y, edge, edge, 1);
    r = gd_search(open, limit, y, edge, edge, 1);
    *left = l < 0 ? 0 : l + 1;
    *right = r < 0 ? limit : r - 1;
    return 1;
}

void gd_move_rows(unsigned to, unsigned from, unsigned count)
{
    memmove(pixel_at(0, to), pixel_at(0, from), (size_t)count * width);
    touch(to, count);
}

void gd_glyph(unsigned x, unsigned y, const u8 *bits, unsigned rows, unsigned foreground)
{
    unsigned line, bit;

    for (line = 0; line < rows; line++)
        for (bit = 0; bit < HOST_CELL_WIDTH; bit++)
            blend(x + bit, y + line, bits[line] & 0x80 >> bit ? foreground : 0, 0);
    touch(y, rows);
}

const u8 *gd_font(unsigned rows)
{
    return rows == FONT_ROWS_14 ? qb_vga_font_14 : rows == FONT_ROWS_8 ? qb_vga_font_8 : qb_vga_font_16;
}

/* Fills: one box, dot, line and octet for every kind of mode, over blend(). */
static void host_box(const GdFill *fill, unsigned x, unsigned y, unsigned count, unsigned rows)
{
    unsigned row, column;

    for (row = 0; row < rows; row++)
        for (column = 0; column < count; column++)
            blend(x + column, y + row, fill->color, fill->operation);
    touch(y, rows);
}

static void host_dot(const GdFill *fill, unsigned x, unsigned y)
{
    blend(x, y, fill->color, fill->operation);
    touch(y, 1);
}

/* Bresenham's line as gdline.asm runs it: `count` pixels along the longer side, the decision value stepping the
   other when it is not negative; the pixels of `style`, its high bit first, are drawn. */
static void host_line(const GdFill *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y)
{
    int major = (int)(dx > dy ? dx : dy), minor = (int)(dx > dy ? dy : dx);
    int decision = 4 * minor - major, minor4 = 4 * minor, step4 = 4 * (minor - major);
    int x_major = dx == 0 ? 2 : dx > dy;
    int px = (int)x, py = (int)y, top = py, bottom = py;
    unsigned style = fill->style & 0xFFFF, count = (unsigned)major + 1;

    for (; count; count--) {
        unsigned carry = style >> 15 & 1;

        style = (style << 1 | carry) & 0xFFFF;
        if (carry)
            blend((unsigned)px, (unsigned)py, fill->color, fill->operation);
        if (x_major == 2) {
            py += step_y;
        } else if (x_major == 1) {
            px++;
            if (decision >= 0)
                decision += step4, py += step_y;
            else
                decision += minor4;
        } else {
            py += step_y;
            if (decision >= 0)
                decision += step4, px++;
            else
                decision += minor4;
        }
        top = py < top ? py : top;
        bottom = py > bottom ? py : bottom;
    }
    touch((unsigned)top, (unsigned)(bottom - top + 1));
}

static void host_octet(const GdFill *fill, unsigned cx, unsigned cy, int u, int v, int w, int z)
{
    unsigned color = fill->color;

    blend(cx + (unsigned)u, cy - (unsigned)v, color, 0);
    blend(cx - (unsigned)u, cy - (unsigned)v, color, 0);
    blend(cx + (unsigned)u, cy + (unsigned)v, color, 0);
    blend(cx - (unsigned)u, cy + (unsigned)v, color, 0);
    blend(cx + (unsigned)w, cy - (unsigned)z, color, 0);
    blend(cx - (unsigned)w, cy - (unsigned)z, color, 0);
    blend(cx + (unsigned)w, cy + (unsigned)z, color, 0);
    blend(cx - (unsigned)w, cy + (unsigned)z, color, 0);
    touch(cy - (unsigned)v, 1);
    touch(cy + (unsigned)v, 1);
    touch(cy - (unsigned)z, 1);
    touch(cy + (unsigned)z, 1);
}

void gd_dots_begin(const GdFill *fill)
{
    (void)fill;
}

void gd_dots_end(const GdFill *fill)
{
    (void)fill;
}

void gd_fill_select(GdFill *fill, unsigned color, unsigned operation)
{
    unsigned byte = color & pixel_mask;

    operation &= 3;
    fill->box = host_box;
    fill->dot = host_dot;
    fill->line = host_line;
    fill->octet = host_octet;
    fill->color = color & 0xFF;
    fill->style = 0xFFFF;
    fill->operation = operation;
    fill->keep = operation == 0 ? 0 : operation == 1 ? byte : operation == 2 ? ~byte & 0xFF : 0xFF;
    fill->flip = operation == 1 ? 0 : byte;
}

/* GET and PUT. A row of the array is one run of bits for each plane (the 16-colour modes: bit n of a pixel's colour
   in plane n), or the packed pixels of CGA, or a byte each; the leftmost pixel is the high bit, and the last byte's
   unused bits are zero. */
static unsigned bits_of(void)
{
    return kind == KIND_PACKED4 ? 2 : 1;
}

static unsigned planes_of(void)
{
    return kind == KIND_PLANAR ? PLANES : 1;
}

void gd_get(unsigned x, unsigned y, unsigned box_width, unsigned rows, u8 *out)
{
    unsigned row, column, plane;

    if (kind == KIND_LINEAR) {
        for (row = 0; row < rows; row++, out += box_width)
            memcpy(out, pixel_at(x, y + row), box_width);
        return;
    }
    {
        unsigned bits = bits_of(), planes = planes_of(), bytes = (box_width * bits + 7) >> 3;

        for (row = 0; row < rows; row++, out += planes * bytes) {
            memset(out, 0, planes * bytes);
            for (column = 0; column < box_width; column++) {
                unsigned value = *pixel_at(x + column, y + row);

                for (plane = 0; plane < planes; plane++) {
                    unsigned at = column * bits, piece = kind == KIND_PLANAR ? value >> plane & 1 : value;

                    out[plane * bytes + (at >> 3)] |= (u8)(piece << (8 - bits - (at & 7)));
                }
            }
        }
    }
}

void gd_put(unsigned x, unsigned y, unsigned box_width, unsigned rows, const u8 *in, unsigned operation,
            unsigned invert)
{
    unsigned row, column, plane;
    unsigned bits = kind == KIND_LINEAR ? 8 : bits_of(), planes = planes_of(), bytes = (box_width * bits + 7) >> 3;

    for (row = 0; row < rows; row++, in += planes * bytes) {
        for (column = 0; column < box_width; column++) {
            unsigned value = 0, at = column * bits;

            for (plane = 0; plane < planes; plane++) {
                unsigned piece = in[plane * bytes + (at >> 3)] >> (8 - bits - (at & 7)) & ((1u << bits) - 1);

                value |= kind == KIND_PLANAR ? piece << plane : piece;
            }
            if (invert)
                value ^= pixel_mask;
            blend(x + column, y + row, value, invert ? 0 : operation);
        }
        touch(y + row, 1);
    }
}

/* The text screen. */
unsigned dev_text_size(void)
{
    return HOST_TEXT_ROWS << 8 | HOST_TEXT_COLUMNS;
}

unsigned dev_text_cursor(void)
{
    return cursor_row << 8 | cursor_column;
}

void dev_text_move(unsigned row, unsigned column)
{
    host_state_lock();
    cursor_row = row < HOST_TEXT_ROWS ? row : HOST_TEXT_ROWS - 1;
    cursor_column = column < HOST_TEXT_COLUMNS ? column : HOST_TEXT_COLUMNS - 1;
    host_state_unlock();
}

void dev_text_cursor_show(int visible)
{
    host_state_lock();
    cursor_visible = visible != 0;
    host_state_unlock();
}

void dev_text_put(unsigned row, unsigned column, unsigned character, unsigned attribute)
{
    if (row < HOST_TEXT_ROWS && column < HOST_TEXT_COLUMNS) {
        cells[row][column] = (uint16_t)(attribute << 8 | (character & 0xFF));
        if (kind == KIND_TEXT)
            draw_cell(row, column);
    }
}

void dev_text_write(unsigned row, unsigned column, const char *text, unsigned count, unsigned attribute)
{
    while (count--)
        dev_text_put(row, column++, (unsigned char)*text++, attribute);
}

void dev_text_scroll(unsigned top, unsigned bottom, unsigned lines, unsigned attribute)
{
    unsigned rows = bottom - top + 1, row, column;

    if (lines == 0 || lines >= rows)
        lines = rows;
    memmove(cells[top], cells[top + lines], (size_t)(rows - lines) * sizeof cells[0]);
    for (row = bottom + 1 - lines; row <= bottom; row++)
        for (column = 0; column < HOST_TEXT_COLUMNS; column++)
            cells[row][column] = (uint16_t)(attribute << 8 | ' ');
    if (kind == KIND_TEXT)
        for (row = top; row <= bottom; row++)
            for (column = 0; column < HOST_TEXT_COLUMNS; column++)
                draw_cell(row, column);
}

void host_text_row(unsigned row, char out[HOST_TEXT_COLUMNS + 1])
{
    unsigned column;

    for (column = 0; column < HOST_TEXT_COLUMNS; column++)
        out[column] = (char)(cells[row][column] & 0xFF);
    out[HOST_TEXT_COLUMNS] = 0;
}

int host_dump_ppm(const char *path)
{
    FILE *file = fopen(path, "wb");
    uint8_t rgb[256][3];
    unsigned at;

    if (!file)
        return -1;
    host_colors(rgb);
    fprintf(file, "P6\n%u %u\n255\n", width, height);
    for (at = 0; at < width * height; at++)
        fwrite(rgb[host_pixels()[at]], 1, 3, file);
    return fclose(file) ? -1 : 0;
}
