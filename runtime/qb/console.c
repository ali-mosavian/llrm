/* The console driver for PRINT (QB rt/iotty.asm B$TTY_SOUT, rt/out.asm B$OUTCNT).  Output goes to
   DOS's standard output, which is the screen or a redirect alike; the cursor column is kept here. */
#include "console.h"
#include "llrm_os.h"

enum { WIDTH = 80, CHUNK = 64 };

static byte column;
static char pending[CHUNK];
static word used;

static void flush(void)
{
    if (used) {
        llrm_os_write_file(LLRM_OS_STDOUT, (const unsigned char __far *)pending, used);
        used = 0;
    }
}

static void put(char c)
{
    if (used == CHUNK)
        flush();
    pending[used++] = c;
    if (c == '\r' || c == '\n' || ++column == WIDTH)
        column = 0;
}

byte cn_pos(void)
{
    return column;
}

byte cn_width(void)
{
    return WIDTH;
}

void cn_putc(char c)
{
    put(c);
    flush();
}

void cn_write(
    const char *s,
    word n)
{
    while (n--)
        put(*s++);
    flush();
}

void cn_crlf(void)
{
    cn_write("\r\n", 2);
}
