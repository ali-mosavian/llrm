/* The keyboard: the OS layer's console keys, and the BIOS to look at the next key without reading it. */
#include "device.h"
#include "llrm_os.h"

enum { BIOS_PEEK_KEY = 0x0100, BIOS_READ_KEY = 0, ZERO_FLAG = 0x40 };

int dev_key_ready(void)
{
    return llrm_os_console_key_ready();
}

unsigned char dev_key_read(void)
{
    return llrm_os_console_read_key();
}

int dev_key_peek_scan(void)
{
    Regs r;

    r.rax = BIOS_PEEK_KEY;
    dev_int16(&r);
    if ((r.rflags & ZERO_FLAG) || (r.rax & 0xFF) != 0)
        return -1;
    return (int)(r.rax >> 8);
}

void dev_key_discard(void)
{
    Regs r;

    r.rax = BIOS_READ_KEY;
    dev_int16(&r);
}
