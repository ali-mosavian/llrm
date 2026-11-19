/* ON ERROR GOTO on a host: B$OEGA keeps the registration, and an error that reaches a handler lands in it (qb_land
   in startup.c, through the frame below). */
#include "error.h"

extern void on_error(unsigned target);
extern void raise(unsigned n);

void B_OEGA(void *pad)
{
    on_error(pad != 0);
}

/* B$OEGP: ON LOCAL ERROR GOTO, with the handler's address or 0. A function with a handler of either kind registers its
   frame at entry, and an error lands in the innermost one, so the registration is the same switch as B$OEGA's. */
void B_OEGP(unsigned pad)
{
    on_error(pad != 0);
}

void B_SERR(short number)
{
    raise((unsigned short)number);
}

short B_FERR(void)
{
    return (short)b_errnum;
}

void B_RESN(void)
{
    qb_end();
}

/* The landing (qb_land in startup.c): a function with an ON ERROR handler registers a jmp_buf here at entry and takes
   it back before it returns (crates/frontends/qb/src/arm64.rs, `framed`). */
extern void *qb_host_frame_top;

void *qb_host_frame(void *frame)
{
    void *before = qb_host_frame_top;

    qb_host_frame_top = frame;
    return before;
}

/* Hands a stack slot to code the optimizer cannot see into, so the slot lives in memory across a landing. */
void qb_host_escape(void *slot)
{
    (void)slot;
}
