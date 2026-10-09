/* The console driver for PRINT (QB rt/iotty.asm B$TTY_SOUT, rt/out.asm
   B$OUTCNT).  On the screen it writes cells; elsewhere it writes the OS's
   standard output, and the cursor column is all it keeps. */
#include "console.h"
#include "llrm_os.h"

enum {
    STREAM_WIDTH = 80,
    CHUNK = 64,
    TAB_STOP = 8,
    DEFAULT_ATTRIBUTE = 7,
    MAX_FOREGROUND = 31,
    MAX_BACKGROUND = 7,
    BLINK = 0x80,
    HOME = 11,
    CURSOR_RIGHT = 28,
    CURSOR_LEFT = 29,
    CURSOR_UP = 30,
    CURSOR_DOWN = 31
};

static int started;
static int on_screen;
static byte column, row, columns, rows;
static byte attribute = DEFAULT_ATTRIBUTE;
static byte view_top, view_bottom;
static char pending[CHUNK];
static unsigned used;

/* Finds out where output goes, the first time anything is written. */
static void start(void)
{
    unsigned size, cursor;

    started = 1;
    on_screen = llrm_os_screen_is_console();
    columns = STREAM_WIDTH;
    if (!on_screen)
        return;
    size = llrm_os_screen_size();
    cursor = llrm_os_screen_cursor();
    columns = (byte)size;
    rows = (byte)(size >> 8);
    row = (byte)(cursor >> 8);
    column = (byte)cursor;
    view_top = 0;
    view_bottom = rows - 2;   /* the last row is the key line's */
}

static void flush(void)
{
    os_data text = (os_data)pending;

    if (used) {
        llrm_os_write_file(LLRM_OS_STDOUT, text, used);
        used = 0;
    }
}

static void put_stream(char c)
{
    if (used == CHUNK)
        flush();
    pending[used++] = c;
    if (c == '\r' || c == '\n' || ++column == columns)
        column = 0;
}

/* A line is full when the cursor is at `columns`; the next character, not the
   one that filled it, starts the next line.

   The next line of the PRINT window.  The window scrolls when the cursor is at
   its bottom or below it (LOCATE can put it on the last row of the screen,
   which the window leaves out), and the cursor is on the bottom row after. */
static void next_line(void)
{
    column = 0;
    if (row < view_bottom) {
        row++;
    } else {
        llrm_os_screen_scroll(view_top, view_bottom, 1, attribute);
        row = view_bottom;
    }
}

static void clear_window(void)
{
    llrm_os_screen_scroll(view_top, view_bottom, 0, attribute);
    row = view_top;
    column = 0;
}

static void put_cell(char c)
{
    switch (c) {
    case '\r':
    case '\n':
        next_line();
        break;
    case '\f':
        clear_window();
        break;
    case '\t':
        do
            put_cell(' ');
        while (column % TAB_STOP);
        break;
    case '\a':
        break;
    case HOME:
        row = view_top;
        column = 0;
        break;
    case CURSOR_RIGHT:
        if (column < columns - 1)
            column++;
        break;
    case CURSOR_LEFT:
        if (column)
            column--;
        break;
    case CURSOR_UP:
        if (row > view_top && row <= view_bottom)
            row--;
        break;
    case CURSOR_DOWN:
        if (row < view_bottom)
            row++;
        break;
    default:
        if (column == columns)
            next_line();
        llrm_os_screen_put(row, column, (byte)c, attribute);
        column++;
    }
}

static void put(char c)
{
    if (!started)
        start();
    if (on_screen)
        put_cell(c);
    else
        put_stream(c);
}

static void finish(void)
{
    if (on_screen)
        llrm_os_screen_move(row, column < columns ? column : columns - 1);
    else
        flush();
}

byte cn_pos(void)
{
    if (!started)
        start();
    return column;
}

byte cn_width(void)
{
    if (!started)
        start();
    return columns;
}

void cn_putc(char c)
{
    put(c);
    finish();
}

void cn_write(const char *s, unsigned n)
{
    while (n--)
        put(*s++);
    finish();
}

void cn_crlf(void)
{
    if (!started)
        start();
    if (on_screen) {
        next_line();
        finish();
    } else {
        cn_write("\r\n", 2);
    }
}

void cn_color(int foreground, int background)
{
    if (!started)
        start();
    if (foreground > MAX_FOREGROUND || background > MAX_BACKGROUND)
        qb_error(BE_ILLFUN);
    if (foreground >= 0)
        attribute = (attribute & 0x70) | (foreground & 15)
                  | ((foreground & 16) ? BLINK : 0);
    if (background >= 0)
        attribute = (attribute & 0x8F) | (background << 4);
}

void cn_locate(int new_row, int new_column)
{
    if (!started)
        start();
    if (!on_screen)
        return;
    if (new_row == 0 || new_row > rows || new_column == 0
        || new_column > columns)
        qb_error(BE_ILLFUN);
    if (new_row > 0)
        row = new_row - 1;
    if (new_column > 0)
        column = new_column - 1;
    finish();
}

void cn_cls(void)
{
    if (!started)
        start();
    if (!on_screen)
        return;
    clear_window();
    finish();
}

void cn_view(int top, int bottom)
{
    if (!started)
        start();
    if (!on_screen)
        return;
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
    finish();
}

void cn_set_size(int new_columns, int new_rows)
{
    if (!started)
        start();
    if (!on_screen)
        return;
    if ((new_columns && new_columns != columns)
        || (new_rows && new_rows != rows))
        qb_error(BE_ILLFUN);
}
