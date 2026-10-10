/* Whether standard output is the screen (DOS's device information, the handle's
   bits 7 and 1). */
#include "device.h"

enum {
    DOS_IOCTL = 0x4400,
    STDOUT_HANDLE = 1,
    DEVICE_BIT = 0x80,
    CONSOLE_OUT_BIT = 0x02,
    CARRY = 1
};

int dev_stdout_is_screen(void)
{
    Regs r;

    r.rax = DOS_IOCTL;
    r.rbx = STDOUT_HANDLE;
    dev_int21(&r);
    return !(r.rflags & CARRY)
           && (r.rdx & (DEVICE_BIT | CONSOLE_OUT_BIT))
              == (DEVICE_BIT | CONSOLE_OUT_BIT);
}
