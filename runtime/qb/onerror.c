/* ON ERROR GOTO (QB rt/error.asm B$OEGA, B$SERR): what a program with the
   statement has linked, and error.c does without. */
#include "error.h"
#include "module.h"

extern void qb_land(void);
#pragma aux qb_land "QB_LAND"
extern void (*qb_lander)(void);

/* B$OEGA: the handler's offset in the module's code, or 0 for none.  A handler
   set again is how the compiled RESUME ends the error: ERR is 0 after it. */
void on_error(unsigned target)
{
    md_set_on_error(module_data(), target);
    b_inonerr = 0;
    if (target)
        b_errnum = 0;
}

/* B$SERR: ERROR n; 0 and numbers past 255 are Illegal function call. */
void raise(unsigned n)
{
    if (n == 0 || n > 255)
        n = BE_ILLFUN;
    qb_error(n);
}

static void land(void)
{
    qb_land();
}

#define XI_FN onerror_xinit
#include "xi.h"
void onerror_xinit(void)
{
    qb_lander = land;
}
#pragma aux on_error "@on_error@2"
#pragma aux raise "@raise@2"
