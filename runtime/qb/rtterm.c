/* Termination (QB rt/rtterm.asm): END and the end of the module run the end slots, which close
   files, then the term slots, then leave for DOS. */
#include "rtinit.h"
#include "error.h"
#include "llrm_os.h"

static void finish(void)
{
    b_inonerr = 0xFFFF;
    if (qb_rt_inited())
        qb_dispatch(V_END);
    qb_dispatch(V_TERM);
    llrm_os_exit(0);
}

/* B$CEND: SYSTEM and END. */
void QB B_CEND(void)
{
    b_errnum = 0;
    finish();
}

/* B$CENP: the end of the module; an ON ERROR handler still running is a No RESUME error. */
void QB B_CENP(void)
{
    if (b_inonerr)
        qb_no_resume();
    B_CEND();
}
#pragma aux B_CEND "B$CEND"
#pragma aux B_CENP "B$CENP"
