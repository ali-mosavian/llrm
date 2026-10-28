/* FRE: how much memory is left (QB rt/stfree.asm). */
#include "ad.h"
#include "nheap.h"
#include "nhstutil.h"
#include "qb.h"

#pragma aux llrm_os_stack_low "LL$STACK_LOW"
#pragma aux B_FRSD "B$FRSD"
#pragma aux B_FRI2 "B$FRI2"

/* The lowest stack address a checked function may reach (the OS layer's). */
extern uword llrm_os_stack_low;

static long stack_free(void)
{
    char here;

    return (uword)&here - llrm_os_stack_low;
}

/* B$FRSD: FRE(string), the free string space. */
long B_FRSD(SD *sd)
{
    str_tmp_free(sd);
    nh_compact();
    return str_free_bytes();
}

/* B$FRI2: FRE(0) is the free string space too, FRE(-1) the largest array the
   far heap could take and FRE(-2) the unused stack. */
long B_FRI2(short selector)
{
    if (selector == -1)
        return ad_free_bytes();
    if (selector == -2)
        return stack_free();
    nh_compact();
    return str_free_bytes();
}
