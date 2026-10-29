/* The console for PRINT (QB rt/iotty.asm B$TTY_SOUT, rt/out.asm B$OUTCNT). What
   each operation does depends on where output goes, so the console holds a
   table of operation records, one entry per kind of output, as mgl does with
   its device contexts: this file has the OS's standard output when it is a file
   or a pipe, and cntext.c the text screen, linked only by a program that uses
   the screen statements. */
#include "cndriver.h"
#include "dev.h"
#include "llrm_os.h"

#define DAY 8640000L
enum { STREAM_WIDTH = 80, CHUNK = 64, BEEP_HERTZ = 800, BEEP_LENGTH = 25 /* hundredths of a second */ };

const Driver *cn_text_driver;
const Driver *cn_gfx_driver;
byte cn_graphics;
static const Driver stream_driver;
static const Driver *driver;
/* PRINT #n: the file driver, until the statement ends */
static const Driver *redirected;
static byte column;
static char pending[CHUNK];
static unsigned used;

/* The bell: a tone for a quarter of a second, on the text screen and on the graphics screen alike. */
void cn_bell(void)
{
    long start = dev_clock();

    dev_tone(BEEP_HERTZ);
    while ((dev_clock() - start + DAY) % DAY < BEEP_LENGTH)
        ;
    dev_tone(0);
}

static void flush(void)
{
    os_data text = (os_data)pending;

    if (used) {
        llrm_os_write_file(LLRM_OS_STDOUT, text, used);
        used = 0;
    }
}

static void stream_init(void)
{
}

static void stream_write(const char *text, unsigned count)
{
    while (count--) {
        char c = *text++;

        if (used == CHUNK)
            flush();
        pending[used++] = c;
        if (c == '\r' || c == '\n' || ++column == STREAM_WIDTH)
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

static void stream_sync(void)
{
}

static void stream_cursor(int visible)
{
    (void)visible;
}

static byte stream_pos(void)
{
    return column;
}

/* A stream has one line, as far as CSRLIN can tell. */
static byte stream_line(void)
{
    return 0;
}

static byte stream_width(void)
{
    return STREAM_WIDTH;
}

/* COLOR is checked even where it has no screen to colour. */
static void stream_color(int foreground, int background)
{
    if (foreground > 31 || background > 15)
        qb_error(BE_ILLFUN);
}

static void stream_locate(int row, int column, int cursor)
{
    (void)row;
    (void)column;
    (void)cursor;
}

static void stream_clear(void)
{
}

static void stream_view(int top, int bottom)
{
    (void)top;
    (void)bottom;
}

static void stream_size(int columns, int rows)
{
    (void)columns;
    (void)rows;
}

static const Driver stream_driver = {stream_init,  stream_write,  stream_newline, stream_erase,
                                     stream_sync,  stream_cursor, stream_pos,     stream_line,
                                     stream_width, stream_color,  stream_locate,  stream_clear,
                                     stream_view,  stream_size};

const Driver *cn_driver(void)
{
    if (redirected)
        return redirected;
    if (!driver) {
        if (!dev_stdout_is_screen())
            driver = &stream_driver;
        else if (cn_graphics && cn_gfx_driver)
            driver = cn_gfx_driver;
        else if (cn_text_driver)
            driver = cn_text_driver;
        else
            driver = &stream_driver;
        driver->init();
    }
    return driver;
}

/* PRINT # sends the statement's output to a file's driver, which it ends. */
void cn_redirect(const Driver *to)
{
    redirected = to;
}

void cn_reset_output(void)
{
    redirected = 0;
}

/* The screen mode changed: the next output looks again. */
void cn_mode_changed(void)
{
    driver = 0;
}

byte cn_pos(void)
{
    return cn_driver()->pos();
}

byte cn_line(void)
{
    return cn_driver()->line();
}

byte cn_width(void)
{
    return cn_driver()->width();
}

/* A NUL prints nothing, on the screen or redirected (BCOM45, measured: "a" + CHR$(0) + "b" prints ab). */
void cn_putc(char c)
{
    if (c)
        cn_driver()->write(&c, 1);
}

void cn_write(const char *s, unsigned n)
{
    const char *start = s;

    while (n) {
        if (*s == 0) {
            if (s != start)
                cn_driver()->write(start, s - start);
            start = s + 1;
        }
        s++;
        n--;
    }
    if (s != start)
        cn_driver()->write(start, s - start);
}

void cn_crlf(void)
{
    cn_driver()->newline();
}

void cn_erase(void)
{
    cn_driver()->erase();
}

void cn_sync(void)
{
    cn_driver()->sync();
}

void cn_waiting(int waiting)
{
    cn_driver()->cursor(waiting);
}

void cn_color(int foreground, int background)
{
    cn_driver()->color(foreground, background);
}

void cn_locate(int row, int column, int cursor)
{
    cn_driver()->locate(row, column, cursor);
}

void cn_cls(void)
{
    const Driver *current = cn_driver();

    current->clear();
    /* the pixels are the screen's whatever standard output is: CLS cleared them under a redirected one in the MS runtime */
    if (cn_graphics && cn_gfx_driver && current != cn_gfx_driver)
        cn_gfx_driver->clear();
}

void cn_view(int top, int bottom)
{
    cn_driver()->view(top, bottom);
}

void cn_set_size(int columns, int rows)
{
    cn_driver()->size(columns, rows);
}
