/* What the portable startup (runtime/qb/rtinit.c, nheap.c) asks of a host: the initializers (xi.h runs them from
   constructors, so there is nothing to walk), and the room the heaps share. */
#include <setjmp.h>
#include "llrm_os.h"
#include "qb.h"
#include "rtinit.h"
#include "shell.h"

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

/* ON ERROR's landing: back into the innermost function that has a handler (onerror_host.c); without one the program
   ends. */
void *qb_host_frame_top;

#ifdef QB_HOST_UNWINDS
/* The host throws, and the `catch_all` of the nearest function with a handler takes it (wasm exception handling). */
extern _Noreturn void llrm_host_land(void);
#endif

void qb_land(void)
{
#ifdef QB_HOST_UNWINDS
    llrm_host_land();
#endif
    if (qb_host_frame_top)
        longjmp(*(jmp_buf *)qb_host_frame_top, 1);
    llrm_os_exit(1);
}

/* SHELL's memory: a host's child runs in its own process, so there is nothing to give up or take back. */
void qb_shell_release(void)
{
}

void qb_shell_reclaim(void)
{
}
