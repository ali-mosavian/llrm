/* PRINT on the graphics screen (QB rt/llscnio.asm GraphOutStr): a character
   cell of the BIOS's font, 8 pixels wide and `cell_height` high, drawn in the
   foreground over the background; the screen statements for text. */
#include "cndriver.h"
#include "gfx.h"
#include "gfxdev.h"

enum { CELL_WIDTH = 8, TAB_STOP = 8, BELL = 7, HOME = 11 };

static byte column, row, columns, rows;
static byte view_top, view_bottom;
static const u8 QB_FAR *font;

static void gfx_init(void)
{
    columns = gfx_current->width / CELL_WIDTH;
    rows = gfx_current->height / gfx_current->cell_height;
    row = column = 0;
    view_top = 0;
    view_bottom = rows - 2;   /* the last row is the key line's */
    font = gd_font(gfx_current->cell_height);
}

static void draw(byte at_row, byte at_column, byte c)
{
    unsigned height = gfx_current->cell_height;

    gd_glyph(at_column * CELL_WIDTH, at_row * height,
             font + (unsigned)c * height, height, gfx_foreground);
}

static void clear_rows(byte first, byte last)
{
    unsigned y, height = gfx_current->cell_height;

    for (y = first * height; y < (last + 1u) * height; y++)
        gd_span(0, gfx_current->width, y, 0, OP_SET);
}

/* The window scrolls up a text row when the cursor is at its bottom. */
static void gfx_newline(void)
{
    unsigned height = gfx_current->cell_height;

    column = 0;
    if (row < view_bottom) {
        row++;
        return;
    }
    gd_move_rows(view_top * height, (view_top + 1u) * height,
                          (view_bottom - view_top) * height);
    clear_rows(view_bottom, view_bottom);
    row = view_bottom;
}

static void gfx_write(const char *text, unsigned count)
{
    while (count--) {
        byte c = (byte)*text++;

        if (c == '\r' || c == '\n') {
            gfx_newline();
        } else if (c == '\f') {
            clear_rows(view_top, view_bottom);
            row = view_top;
            column = 0;
        } else if (c == HOME) {
            row = view_top;
            column = 0;
        } else if (c == '\t') {
            do {
                if (column == columns)
                    gfx_newline();
                draw(row, column++, ' ');
            } while (column % TAB_STOP);
        } else if (c != BELL) {
            if (column == columns)
                gfx_newline();
            draw(row, column++, c);
        }
    }
}

static void gfx_erase(void)
{
    if (column)
        column--;
    draw(row, column, ' ');
}

static void gfx_sync(void)
{
}

static void gfx_cursor(int visible)
{
    (void)visible;
}

static byte gfx_pos(void)
{
    return column;
}

static byte gfx_line(void)
{
    return row;
}

static byte gfx_width(void)
{
    return columns;
}

static void gfx_color_set(int foreground, int background)
{
    if (foreground > 31 || background > 15)
        qb_error(BE_ILLFUN);
    if (foreground >= 0)
        gfx_foreground = foreground & 15;
    if (background >= 0)
        gfx_set_background(background & 15);
}

static void gfx_locate(int new_row, int new_column, int cursor)
{
    (void)cursor;
    if (new_row == 0 || new_row > rows || new_column == 0
        || new_column > columns)
        qb_error(BE_ILLFUN);
    if (new_row > 0)
        row = new_row - 1;
    if (new_column > 0)
        column = new_column - 1;
}

static void gfx_clear_text(void)
{
    gfx_clear();
    row = view_top;
    column = 0;
}

static void gfx_view(int top, int bottom)
{
    if (top == -1 && bottom == -1) {
        view_top = 0;
        view_bottom = rows - 1;
    } else {
        if (top < 1 || bottom < top || bottom > rows)
            qb_error(BE_ILLFUN);
        view_top = top - 1;
        view_bottom = bottom - 1;
    }
    row = view_top;
    column = 0;
}

static void gfx_size(int new_columns, int new_rows)
{
    (void)new_columns;
    (void)new_rows;
}

static const Driver driver = {
    gfx_init, gfx_write, gfx_newline, gfx_erase, gfx_sync, gfx_cursor,
    gfx_pos, gfx_line, gfx_width, gfx_color_set, gfx_locate, gfx_clear_text, gfx_view,
    gfx_size
};

#define XI_FN cn_gfx_xinit
#include "xi.h"
void cn_gfx_xinit(void)
{
    cn_gfx_driver = &driver;
}
