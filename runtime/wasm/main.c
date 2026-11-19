/* A compiled QB program on the headless device, started by the host: `llrm_wasm_start` is what the node
   harness calls after the constructors; the host types keys with headless_type, reads the screen
   (host_pixels, host_colors) and ends with the program's exit. */
#include "headless.h"
#include "llrm_os.h"
#include "qb.h"

extern int qb_host_program(int argc, char **argv);

int llrm_wasm_start(void)
{
    headless_reset();
    headless_set_real_time(1);
    return qb_host_program(0, 0);
}

/* The engine ran out of call stack under the program (a RangeError in the host): QB's fatal "Out of stack space",
   as the runtime's own stack check ends a program. Called by the host after the engine has unwound. */
void llrm_wasm_stack_overflow(void)
{
    qb_error(FE_NOSTACK);
}
