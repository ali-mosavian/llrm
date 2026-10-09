/* Startup (QB rt/rtinit.asm B$Init, B$COMP_DISP): run the XI initializers,
   which register their components, then the ini slots. */
#include "rtinit.h"
#include "startup.h"

static Comp *comps;
static byte rt_inited;

void qb_comp_add(Comp *c)
{
    Comp **at = &comps;

    while (*at && (*at)->id < c->id)
        at = &(*at)->next;
    c->next = *at;
    *at = c;
}

static void run_down(Comp *c, byte slot)
{
    if (c) {
        run_down(c->next, slot);
        if (c->v[slot])
            c->v[slot]();
    }
}

/* B$COMP_DISP: the slot of every component, in order for the starting ones and
   backwards for the ending ones. */
void qb_dispatch(byte slot)
{
    Comp *c;

    if (slot >= V_END)
        run_down(comps, slot);
    else
        for (c = comps; c; c = c->next)
            if (c->v[slot])
                c->v[slot]();
}

/* True once startup has finished: termination before it must not run the end
   slots. */
byte qb_rt_inited(void)
{
    return rt_inited;
}

void __cdecl qb_start(void)
{
    qb_run_initializers();
    qb_dispatch(V_INI);
    rt_inited = 1;
}

/* Where the next item of a line of INPUT is, while one is waiting (input.c and
   read.c share it without calling each other). */
const char *qb_input_line;
