/* The clock: DOS function 2Ah to 2Dh. */
#include "device.h"

enum {
    DOS_GET_DATE = 0x2A00,
    DOS_SET_DATE = 0x2B00,
    DOS_GET_TIME = 0x2C00,
    DOS_SET_TIME = 0x2D00,
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
    seconds =
        ((r.rcx >> 8) * MINUTES_PER_HOUR + (r.rcx & 0xFF)) * SECONDS_PER_MINUTE + (r.rdx >> 8);
    return seconds * HUNDREDTHS + (r.rdx & 0xFF);
}

void dev_date(unsigned *year, unsigned *month, unsigned *day)
{
    Regs r;

    r.rax = DOS_GET_DATE;
    dev_int21(&r);
    *year = (unsigned)r.rcx;
    *month = (unsigned)(r.rdx >> 8);
    *day = (unsigned)(r.rdx & 0xFF);
}

void dev_time(unsigned *hour, unsigned *minute, unsigned *second)
{
    Regs r;

    r.rax = DOS_GET_TIME;
    dev_int21(&r);
    *hour = (unsigned)(r.rcx >> 8);
    *minute = (unsigned)(r.rcx & 0xFF);
    *second = (unsigned)(r.rdx >> 8);
}

int dev_date_set(unsigned year, unsigned month, unsigned day)
{
    Regs r;

    r.rax = DOS_SET_DATE;
    r.rcx = year;
    r.rdx = month << 8 | day;
    dev_int21(&r);
    return (r.rax & 0xFF) != 0;
}

int dev_time_set(unsigned hour, unsigned minute, unsigned second)
{
    Regs r;

    r.rax = DOS_SET_TIME;
    r.rcx = hour << 8 | minute;
    r.rdx = second << 8;
    dev_int21(&r);
    return (r.rax & 0xFF) != 0;
}
