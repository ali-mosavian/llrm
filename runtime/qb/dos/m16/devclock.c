/* The time of day (DOS function 2Ch). */
#include "device.h"

enum {
    DOS_GET_TIME = 0x2C00,
    MINUTES_PER_HOUR = 60,
    SECONDS_PER_MINUTE = 60,
    HUNDREDTHS = 100
};

long dev_clock(void)
{
    Regs r;
    long seconds;

    r.rax = DOS_GET_TIME;
    dev_int21(&r);
    seconds = ((r.rcx >> 8) * MINUTES_PER_HOUR + (r.rcx & 0xFF))
              * SECONDS_PER_MINUTE + (r.rdx >> 8);
    return seconds * HUNDREDTHS + (r.rdx & 0xFF);
}
