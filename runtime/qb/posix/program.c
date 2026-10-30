/* A compiled QB program as the host's program: start the runtime, run the module. A module ends through B$CENP,
   which leaves through llrm_os_exit. */
#include "llrm_os.h"
#include "qb.h"
#include "rtinit.h"

extern void qb_module_run(void);

int qb_host_program(int argc, char **argv)
{
    (void)argc;
    (void)argv;
    qb_start();
    qb_module_run();
    return 0;
}
