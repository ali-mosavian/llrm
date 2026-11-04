/* Included in every wasm32 runtime source (build.sh): the host's poll, which moves its clock and runs the tick. */
#ifndef LLRM_WASM_IDLE_H
#define LLRM_WASM_IDLE_H
void llrm_host_poll(void);
#endif
