/* What the portable startup (runtime/qb/rtinit.c, nheap.c) asks of this target:
   the initializers the linker gathered (xi.h), and the dynamic region of
   DGROUP. */
#include "qb.h"
#include "rtinit.h"

/* start.asm: where XIB and XIE are in DGROUP, the end of the stack, and the
   last usable word of the group. */
extern unsigned qb_xi_begin, qb_xi_end, qb_asizds;
extern char qb_atopsp;

/* Runs each initializer the linked modules registered (B$Init's XI walk). */
void qb_run_initializers(void)
{
    qb_init_fn *at;

    for (at = (qb_init_fn *)qb_xi_begin; (unsigned)at < qb_xi_end; at++)
        if (*at)
            (*at)();
}

/* The room the heaps share: everything above the stack, to the last word of
   DGROUP. */
void qb_dynamic_region(char **first, char **top)
{
    *first = &qb_atopsp;
    *top = (char *)qb_asizds;
}
