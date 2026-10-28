/* TIMER (QB rt/ostimer.asm B$TIMR). */
#include "dev.h"
#include "qb.h"

enum { TENTHS_OF_A_MILLISECOND = 10000 };

static float seconds;

#pragma aux B_TIMR "B$TIMR"

/* B$TIMR: the seconds since midnight, a SINGLE whose address is returned. */
float *B_TIMR(void)
{
    seconds = (float)((double)dev_clock_fine() / TENTHS_OF_A_MILLISECOND);
    return &seconds;
}
