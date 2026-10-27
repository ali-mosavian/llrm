/* The printer (QB rt/iolpt.asm): the BIOS's status of the first one. */
#include "device.h"

enum { PRINTER_STATUS = 0x0200, NOT_BUSY = 0x80, SELECTED = 0x10, FAULTS = 0x29 };

int dev_printer_ready(void)
{
    Regs regs;

    regs.rax = PRINTER_STATUS;
    regs.rdx = 0;
    dev_int17(&regs);
    return (regs.rax >> 8 & (NOT_BUSY | SELECTED | FAULTS)) == (NOT_BUSY | SELECTED);
}
