/* What the portable startup (runtime/qb/rtinit.c, nheap.c) asks of a host: the initializers (xi.h runs them from
   constructors, so there is nothing to walk), and the room the heaps share. */
#include "llrm_os.h"
#include "qb.h"
#include "rtinit.h"

/* The lowest stack address a checked function may reach: FRE(-2) counts down to it. */
uword llrm_os_stack_low;

void qb_run_initializers(void)
{
    char here;

    llrm_os_stack_low = (uword)&here - (6UL << 20);
}

void qb_dynamic_region(char **first, char **top)
{
    static const unsigned long sizes[] = {64UL << 20, 16UL << 20, 4UL << 20, 1UL << 20};
    unsigned at;

    for (at = 0; at < sizeof sizes / sizeof sizes[0]; at++) {
        char *room = (char *)llrm_os_more(sizes[at]);

        if (room) {
            *first = room;
            *top = room + sizes[at];
            return;
        }
    }
    *first = *top = 0;
}

/* ON ERROR's landing needs the frame to be put back; the host has no frames yet (docs/working/qb-host.md, open
   rows), so an error that reaches a handler ends the program. */
void qb_land(void)
{
    llrm_os_exit(1);
}
