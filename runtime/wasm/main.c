/* A compiled QB program on the headless device, started by the host: `llrm_wasm_start` is what the page or the node
   harness calls after the constructors; the host types keys with headless_type, reads the screen
   (host_pixels, host_colors) and ends with the program's exit. */
#include "headless.h"
#include "llrm_os.h"

extern int qb_host_program(int argc, char **argv);

int llrm_wasm_start(void)
{
    headless_reset();
    headless_set_real_time(1);
    return qb_host_program(0, 0);
}
