/* The console driver for PRINT (QB rt/iotty.asm B$TTY_SOUT, rt/out.asm B$OUTCNT).  Output goes to
   DOS's standard output, which is the screen or a redirect alike; the cursor column is kept here. */
#include "console.h"
#include "llrm_os.h"

enum { WIDTH = 80, CHUNK = 64 };

static byte col;
static byte buf[CHUNK];
static word used;

static void flush(void)
{
    if (used) {
        llrm_os_write_file(LLRM_OS_STDOUT, (const unsigned char __far *)buf, used);
        used = 0;
    }
}

static void put(byte c)
{
    if (used == CHUNK)
        flush();
    buf[used++] = c;
}

byte cn_pos(void)
{
    return col;
}

byte cn_width(void)
{
    return WIDTH;
}

void cn_putc(byte c)
{
    put(c);
    if (c == '\r' || c == '\n')
        col = 0;
    else if (++col == WIDTH)
        col = 0;
    flush();
}

void cn_write(const byte *s, word n)
{
    for (; n; n--, s++) {
        put(*s);
        if (*s == '\r' || *s == '\n')
            col = 0;
        else if (++col == WIDTH)
            col = 0;
    }
    flush();
}

void cn_crlf(void)
{
    cn_write((const byte *)"\r\n", 2);
}
