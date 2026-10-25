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

/* The BIOS tick count (18.2 a second) and the timer chip's count within the tick, read together: a tick that comes between
   the two reads is read again. The chip is in square-wave mode, which counts down by two, 65536 to 0 in each half of the
   tick (838 ns a step); the status's output pin says which half. */
enum {
    BIOS_TICK_COUNT = 0x6C,
    PIT_CHANNEL_0 = 0x40,
    PIT_CONTROL = 0x43,
    PIT_READ_BACK_COUNT_AND_STATUS_0 = 0xC2,
    PIT_OUT_HIGH = 0x80,
    PIT_STEPS = 65536UL,
    TENTHS_MS_PER_TICK_WHOLE = 549,
    TENTHS_MS_PER_TICK_FRACTION = 2542 /* ten thousandths: 65536 steps at 1193181.67 Hz is 549.2542 */
};

static const volatile u8 QB_FAR *const bios_data = (const volatile u8 QB_FAR *)0x400UL;

static unsigned long bios_ticks(void)
{
    return (unsigned long)bios_data[BIOS_TICK_COUNT] | (unsigned long)bios_data[BIOS_TICK_COUNT + 1] << 8
         | (unsigned long)bios_data[BIOS_TICK_COUNT + 2] << 16 | (unsigned long)bios_data[BIOS_TICK_COUNT + 3] << 24;
}

long dev_clock_fine(void)
{
    static long last;
    unsigned long before, after, left, status;
    long now;

    do {
        before = bios_ticks();
        dev_outb(PIT_CONTROL, PIT_READ_BACK_COUNT_AND_STATUS_0);
        status = dev_inb(PIT_CHANNEL_0);
        left = dev_inb(PIT_CHANNEL_0);
        left |= (unsigned long)dev_inb(PIT_CHANNEL_0) << 8;
        after = bios_ticks();
    } while (before != after);
    if (left == 0)
        left = PIT_STEPS;
    now = (long)(before * TENTHS_MS_PER_TICK_WHOLE + before * TENTHS_MS_PER_TICK_FRACTION / 10000
                 + ((status & PIT_OUT_HIGH ? 0 : PIT_STEPS) + (PIT_STEPS - left)) * TENTHS_MS_PER_TICK_WHOLE / (2 * PIT_STEPS));
    /* The chip reloads before the tick's interrupt has counted it: for that moment the time is a tick behind. The rounding
       of the two clocks leaves it a little behind at other times. */
    if (now < last)
        now = last - now > TENTHS_MS_PER_TICK_WHOLE / 2 ? now + TENTHS_MS_PER_TICK_WHOLE + 1 : last;
    last = now;
    return now;
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
