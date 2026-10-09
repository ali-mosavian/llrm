/* The console for PRINT (QB rt/iotty.asm B$TTY_SOUT, rt/out.asm B$OUTCNT) and
   the screen statements.  What each does depends on where output goes, so the
   console holds a table of operation records, one entry per kind of output, and
   indexes it with the kind in use, as mgl does with its device contexts: one
   entry for the OS's standard output when it is a file or a pipe, one for the
   text screen.  A screen mode with its own way to draw adds an entry. */
#include "console.h"
#include "llrm_os.h"

enum {
    STREAM_WIDTH = 80,
    CHUNK = 64,
    TAB_STOP = 8,
    DEFAULT_ATTRIBUTE = 7,
    MAX_FOREGROUND = 31,
    MAX_BACKGROUND = 15,       /* above 7 is the same colour */
    BLINK = 0x80,
    HOME = 11,
    CURSOR_RIGHT = 28,
    CURSOR_LEFT = 29,
    CURSOR_UP = 30,
    CURSOR_DOWN = 31
};

enum Kind { KIND_UNSET = -1, KIND_STREAM, KIND_TEXT, KINDS };

typedef struct Driver {
    void (*write)(const char *text, unsigned count);
    void (*newline)(void);
    void (*erase)(void);
    void (*clear)(void);
    void (*locate)(int row, int column);
    void (*view)(int top, int bottom);
    void (*size)(int columns, int rows);
    void (*sync)(void);
} Driver;

static const Driver drivers[KINDS];
static enum Kind kind = KIND_UNSET;
static byte column, row, columns, rows;
static byte attribute = DEFAULT_ATTRIBUTE;
static byte view_top, view_bottom;
static char pending[CHUNK];
static unsigned used;
static int cursor_stale;

/* Finds out where output goes, the first time anything is written. */
static const Driver *start(void)
{
    unsigned size, cursor;

    if (kind != KIND_UNSET)
        return &drivers[kind];
    columns = STREAM_WIDTH;
    if (!llrm_os_screen_is_console()) {
        kind = KIND_STREAM;
        return &drivers[kind];
    }
    size = llrm_os_screen_size();
    cursor = llrm_os_screen_cursor();
    columns = (byte)size;
    rows = (byte)(size >> 8);
    row = (byte)(cursor >> 8);
    column = (byte)cursor;
    view_top = 0;
    view_bottom = rows - 2;   /* the last row is the key line's */
    kind = KIND_TEXT;
    return &drivers[kind];
}

/* Standard output as a file or a pipe: bytes, and the column counted. */

static void flush(void)
{
    os_data text = (os_data)pending;

    if (used) {
        llrm_os_write_file(LLRM_OS_STDOUT, text, used);
        used = 0;
    }
}

static void stream_write(const char *text, unsigned count)
{
    while (count--) {
        char c = *text++;

        if (used == CHUNK)
            flush();
        pending[used++] = c;
        if (c == '\r' || c == '\n' || ++column == columns)
            column = 0;
    }
    flush();
}

static void stream_newline(void)
{
    stream_write("\r\n", 2);
}

static void stream_erase(void)
{
    stream_write("\b \b", 3);
}

static void stream_clear(void)
{
}

static void stream_locate(int new_row, int new_column)
{
    (void)new_row;
    (void)new_column;
}

static void stream_view(int top, int bottom)
{
    (void)top;
    (void)bottom;
}

static void stream_size(int new_columns, int new_rows)
{
    (void)new_columns;
    (void)new_rows;
}

static void stream_sync(void)
{
}

/* The text screen: cells written in video memory, the BIOS asked only to scroll
   and clear. */

/* A line is full when the cursor is at `columns`; the next character, not the
   one that filled it, starts the next line.

   The next line of the PRINT window.  The window scrolls when the cursor is at
   its bottom or below it (LOCATE can put it on the last row of the screen,
   which the window leaves out), and the cursor is on the bottom row after. */
static void text_newline(void)
{
    column = 0;
    if (row < view_bottom) {
        row++;
    } else {
        llrm_os_screen_scroll(view_top, view_bottom, 1, attribute);
        row = view_bottom;
    }
}

static void text_clear(void)
{
    llrm_os_screen_scroll(view_top, view_bottom, 0, attribute);
    row = view_top;
    column = 0;
    cursor_stale = 1;
}

/* A control character of the console. */
static void control(char c)
{
    switch (c) {
    case '\r':
    case '\n':
        text_newline();
        break;
    case '\f':
        text_clear();
        break;
    case '\t':
        do {
            if (column == columns)
                text_newline();
            llrm_os_screen_put(row, column++, ' ', attribute);
        } while (column % TAB_STOP);
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
        break;                      /* the bell is silent here */
    }
}

/* Whether a character is shown as it is: all but the bell, tab, line feed,
   home, form feed, return and the four cursor keys. */
static int plain_char(char c)
{
    byte code = (byte)c;

    return !(code == 7 || (code >= 9 && code <= 13)
             || (code >= 28 && code <= 31));
}

/* Writes the run of plain characters that fits the line, in one call. */
static unsigned put_run(const char *s, unsigned n)
{
    unsigned run = 0;

    if (column == columns)
        text_newline();
    while (run < n && plain_char(s[run]) && column + run < columns)
        run++;
    llrm_os_screen_write(row, column, (os_data)s, run, attribute);
    column += run;
    return run;
}

static void text_write(const char *text, unsigned count)
{
    unsigned taken;

    while (count) {
        if (plain_char(*text)) {
            taken = put_run(text, count);
        } else {
            control(*text);
            taken = 1;
        }
        text += taken;
        count -= taken;
    }
    cursor_stale = 1;
}

static void text_erase(void)
{
    if (column)
        column--;
    llrm_os_screen_put(row, column, ' ', attribute);
    cursor_stale = 1;
}

static void text_locate(int new_row, int new_column)
{
    if (new_row == 0 || new_row > rows || new_column == 0
        || new_column > columns)
        qb_error(BE_ILLFUN);
    if (new_row > 0)
        row = new_row - 1;
    if (new_column > 0)
        column = new_column - 1;
    cursor_stale = 1;
}

static void text_view(int top, int bottom)
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
    cursor_stale = 1;
}

/* Only the screen's own size is accepted. */
static void text_size(int new_columns, int new_rows)
{
    if ((new_columns && new_columns != columns)
        || (new_rows && new_rows != rows))
        qb_error(BE_ILLFUN);
}

/* The hardware cursor follows only when something is going to look at it: a key
   is waited for, or the program ends. */
static void text_sync(void)
{
    if (cursor_stale) {
        llrm_os_screen_move(row, column < columns ? column : columns - 1);
        cursor_stale = 0;
    }
}

static void text_crlf(void)
{
    text_newline();
    cursor_stale = 1;
}

static const Driver drivers[KINDS] = {
    {stream_write, stream_newline, stream_erase, stream_clear,
     stream_locate, stream_view, stream_size, stream_sync},
    {text_write, text_crlf, text_erase, text_clear,
     text_locate, text_view, text_size, text_sync}
};

byte cn_pos(void)
{
    start();
    return column;
}

byte cn_width(void)
{
    start();
    return columns;
}

void cn_putc(char c)
{
    start()->write(&c, 1);
}

void cn_write(const char *s, unsigned n)
{
    start()->write(s, n);
}

void cn_crlf(void)
{
    start()->newline();
}

void cn_erase(void)
{
    start()->erase();
}

void cn_sync(void)
{
    start()->sync();
}

void cn_color(int foreground, int background)
{
    start();
    if (foreground > MAX_FOREGROUND || background > MAX_BACKGROUND)
        qb_error(BE_ILLFUN);
    if (foreground >= 0)
        attribute = (attribute & 0x70) | (foreground & 15)
                  | ((foreground & 16) ? BLINK : 0);
    if (background >= 0)
        attribute = (attribute & 0x8F) | ((background & 7) << 4);
}

void cn_locate(int new_row, int new_column)
{
    start()->locate(new_row, new_column);
}

void cn_cls(void)
{
    start()->clear();
}

void cn_view(int top, int bottom)
{
    start()->view(top, bottom);
}

void cn_set_size(int new_columns, int new_rows)
{
    start()->size(new_columns, new_rows);
}
