/* SLEEP and BEEP (QB rt/gwsleep, rt/llscnio B$BLEEP). */
#include "console.h"
#include "device.h"
#include "llrm_os.h"
#include "qb.h"

enum { HUNDREDTHS = 100, DAY = 8640000L };

static long elapsed(long since)
{
    return (dev_clock() - since + DAY) % DAY;
}

/* B$SLEP: SLEEP seconds, which ends early on a key; 0 waits for a key. */
void B_SLEP(long seconds)
{
    long start = dev_clock();

    while (!llrm_os_console_key_ready()
           && (seconds == 0 || elapsed(start) < seconds * HUNDREDTHS))
        ;
}

/* B$BEEP: the bell character, which the console sounds or passes on. */
void B_BEEP(void)
{
    cn_putc(7);
}
#pragma aux B_SLEP "B$SLEP"
#pragma aux B_BEEP "B$BEEP"
