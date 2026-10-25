/* What the portable startup (runtime/qb/rtinit.c, nheap.c) asks of a host: the initializers (xi.h runs them from
   constructors, so there is nothing to walk), and the room the heaps share. */
#include "llrm_os.h"
#include "qb.h"
#include "rtinit.h"

void qb_run_initializers(void)
{
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
