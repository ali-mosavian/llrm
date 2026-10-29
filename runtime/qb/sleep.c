/* SLEEP and BEEP (QB rt/gwsleep, rt/llscnio B$BLEEP). */
#include "console.h"
#include "dev.h"
#include "llrm_os.h"
#include "qb.h"

#define DAY 8640000L
enum { HUNDREDTHS = 100 };

static long elapsed(long since)
{
    return (dev_clock() - since + DAY) % DAY;
}

#pragma aux B_SLEP "B$SLEP"
#pragma aux B_BEEP "B$BEEP"

/* B$SLEP: SLEEP seconds, which ends early on a key; 0 waits for a key. */
void B_SLEP(long seconds)
{
    long start = dev_clock();

    for (;;) {
        if (cn_key_hook)
            cn_key_hook();
        if (dev_key_ready()) {
            if (!cn_key_hook || !cn_key_hook())
                break;
        } else if (seconds != 0 && elapsed(start) >= seconds * HUNDREDTHS) {
            break;
        }
    }
}

/* B$BEEP: the bell character, which the console sounds or passes on. */
void B_BEEP(void)
{
    cn_putc(7);
}
