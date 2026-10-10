/* What the portable startup (runtime/qb/rtinit.c, nheap.c) asks of this target: the initializers the
   linker gathered (xi.h), and the room the heaps share. */
#include "llrm_os.h"
#include "qb.h"
#include "rtinit.h"

/* start.asm: where XIB and XIE are. */
extern qb_init_fn *qb_xi_begin, *qb_xi_end;
#pragma aux qb_xi_begin "qb_xi_begin"
#pragma aux qb_xi_end "qb_xi_end"

/* trap.asm: takes the divide fault. */
extern void qb_traps(void);
#pragma aux qb_traps "B$TRAPS"

/* Runs each initializer the linked modules registered (B$Init's XI walk). */
void qb_run_initializers(void)
{
    qb_init_fn *at;

    for (at = qb_xi_begin; at < qb_xi_end; at++)
        if (*at)
            (*at)();
    qb_traps();
}

/* The room the string space and the local heap share: what the OS layer's heap will give, the most of
   a few sizes. */
void qb_dynamic_region(char **first, char **top)
{
    static const unsigned long sizes[] = { 4UL << 20, 1UL << 20, 256UL << 10, 64UL << 10 };
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
