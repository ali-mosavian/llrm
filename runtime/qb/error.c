/* The error model (QB rt/erproc.asm, erhandlr.asm, inc/messages.inc): one number per error, a
   dispatch to the components that must reset, then the ON ERROR handler or the fatal message. */
#include "rtinit.h"
#include "error.h"
#include "llrm_os.h"

word b_errnum;
word b_inonerr;

void qb_error(word n)
{
    b_errnum = n;
    qb_dispatch(V_ERR);
    llrm_os_exit(255);
}

void qb_no_resume(void)
{
    qb_error(BE_NORESUME);
}
