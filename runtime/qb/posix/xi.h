/* QB's INITIALIZER on a host: the module runs XI_FN at startup, and only if it is linked. The constructor
   attribute runs it before main (ELF and Mach-O alike); the components it registers are ordered by id, not by
   the order the initializers ran. Define XI_FN, then include this once. */
#include "platform.h"
#define XI_JOIN2(a, b) a##b
#define XI_JOIN(a, b) XI_JOIN2(a, b)
void XI_FN(void);
static void __attribute__((constructor)) XI_JOIN(xi_run_, XI_FN)(void)
{
    XI_FN();
}
