/* The keyboard for event trapping (events.c): the key the program would read
   next, without reading it. */
#include "device.h"
#include "qb.h"

enum { BIOS_PEEK_KEY = 0x0100, BIOS_READ_KEY = 0, ZERO_FLAG = 0x40 };

/* The scan code of the next key if it is an extended one (a function or
   cursor key), else -1; also -1 with no key waiting. */
int ev_next_extended_scan(void)
{
    Regs r;

    r.rax = BIOS_PEEK_KEY;
    dev_int16(&r);
    if ((r.rflags & ZERO_FLAG) || (r.rax & 0xFF) != 0)
        return -1;
    return (int)(r.rax >> 8);
}

/* The next key is read and dropped. */
void ev_discard_key(void)
{
    Regs r;

    r.rax = BIOS_READ_KEY;
    dev_int16(&r);
}
