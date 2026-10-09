/* TIMER (QB rt/ostimer.asm B$TIMR). */
#include "llrm_os.h"
#include "qb.h"

enum { HUNDREDTHS = 100 };

static float seconds;

/* B$TIMR: the seconds since midnight, a SINGLE whose address is returned. */
float *B_TIMR(void)
{
    seconds = (float)((double)llrm_os_clock_hundredths() / HUNDREDTHS);
    return &seconds;
}
#pragma aux B_TIMR "B$TIMR"
