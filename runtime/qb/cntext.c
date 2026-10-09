/* The text screen (QB rt/llscnio.asm, rt/gwscr.asm): cells written in video
   memory, the BIOS asked only to scroll and clear; the screen statements for
   it.  Linked by a program that uses them. */
#include "cndriver.h"
#include "llrm_os.h"
#include "rtinit.h"

enum {
    TAB_STOP = 8,
    DEFAULT_ATTRIBUTE = 7,
    MAX_FOREGROUND = 31,
    MAX_BACKGROUND = 15,       /* above 7 is the same colour */
    BLINK = 0x80,
    BELL = 7,
    HOME = 11,
    BEEP_HERTZ = 800,
    BEEP_LENGTH = 25,        /* hundredths of a second */
    DAY = 8640000L,
    CURSOR_RIGHT = 28,
    CURSOR_LEFT = 29,
    CURSOR_UP = 30,
    CURSOR_DOWN = 31
};

static byte column, row, columns, rows;
static byte attribute = DEFAULT_ATTRIBUTE;
static byte view_top, view_bottom;
static int cursor_stale;
static int cursor_wanted;     /* LOCATE ,,1 */
static int cursor_shown = 1;

static void text_init(void)
{
    unsigned size = llrm_os_screen_size(), cursor = llrm_os_screen_cursor();

    llrm_os_screen_cursor_show(0);
    cursor_shown = 0;
    columns = (byte)size;
    rows = (byte)(size >> 8);
    row = (byte)(cursor >> 8);
    column = (byte)cursor;
    view_top = 0;
    view_bottom = rows - 2;   /* the last row is the key line's */
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

/* The bell: a tone for a quarter of a second. */
static void bell(void)
{
    long start = llrm_os_clock_hundredths();

    llrm_os_speaker_tone(BEEP_HERTZ);
    while ((llrm_os_clock_hundredths() - start + DAY) % DAY < BEEP_LENGTH)
        ;
    llrm_os_speaker_tone(0);
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
    case BELL:
        bell();
        break;
    default:
        break;
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

static byte text_pos(void)
{
    return column;
}

static byte text_width(void)
{
    return columns;
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

static void text_crlf(void)
{
    text_newline();
    cursor_stale = 1;
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

/* The cursor shows while the program waits for typing, or where LOCATE asked
   for it, and not while it runs. */
static void text_cursor(int waiting)
{
    int show = waiting || cursor_wanted;

    if (show != cursor_shown) {
        llrm_os_screen_cursor_show(show);
        cursor_shown = show;
    }
}

static const Driver text_driver = {
    text_init, text_write, text_crlf, text_erase, text_sync, text_cursor,
    text_pos, text_width
};

#define XI_FN cn_text_xinit
#include "xi.h"
void cn_text_xinit(void)
{
    cn_text_driver = &text_driver;
}

/* Whether output is on the screen: the statements do nothing otherwise. */
static int on_screen(void)
{
    return cn_driver() == &text_driver;
}

void cn_color(int foreground, int background)
{
    if (foreground > MAX_FOREGROUND || background > MAX_BACKGROUND)
        qb_error(BE_ILLFUN);
    if (!on_screen())
        return;
    if (foreground >= 0)
        attribute = (attribute & 0x70) | (foreground & 15)
                  | ((foreground & 16) ? BLINK : 0);
    if (background >= 0)
        attribute = (attribute & 0x8F) | ((background & 7) << 4);
}

void cn_locate(int new_row, int new_column, int cursor)
{
    if (!on_screen())
        return;
    text_locate(new_row, new_column);
    if (cursor >= 0) {
        cursor_wanted = cursor != 0;
        text_cursor(0);
    }
}

void cn_cls(void)
{
    if (on_screen())
        text_clear();
}

void cn_view(int top, int bottom)
{
    if (on_screen())
        text_view(top, bottom);
}

void cn_set_size(int new_columns, int new_rows)
{
    if (on_screen())
        text_size(new_columns, new_rows);
}
