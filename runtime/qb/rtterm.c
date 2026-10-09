/* Termination (QB rt/rtterm.asm): END and the end of the module run the end
   slots, which close files, then the term slots, then leave the program. */
#include "console.h"
#include "rtinit.h"
#include "error.h"
#include "llrm_os.h"

static void finish(void)
{
    b_inonerr = 0xFFFF;
    if (qb_rt_inited())
        qb_dispatch(V_END);
    qb_dispatch(V_TERM);
    cn_sync();
    cn_waiting(1);
    llrm_os_exit(0);
}

/* Ends the program as END does, for the console's Ctrl-Z. */
void qb_end(void)
{
    b_errnum = 0;
    finish();
}

/* B$CEND: SYSTEM and END. */
void B_CEND(void)
{
    qb_end();
}

/* B$CENP: the end of the module; an ON ERROR handler still running is a No
   RESUME error. */
void B_CENP(void)
{
    if (b_inonerr)
        qb_no_resume();
    B_CEND();
}
#pragma aux B_CEND "B$CEND"
#pragma aux B_CENP "B$CENP"
