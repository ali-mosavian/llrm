/* ON ERROR GOTO on a host that has no landing yet (docs/working/qb-host.md, open rows): the registration is kept, ERR
   and ERROR work, and an error that reaches a handler ends the program (qb_land in startup.c). A program that never
   raises an error behaves as on DOS. */
#include "error.h"

extern void on_error(unsigned target);
extern void raise(unsigned n);

void B_OEGA(void *pad)
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
