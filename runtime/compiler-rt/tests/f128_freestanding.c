/* Freestanding driver: the binary128 add/sub/mul/div of lib/fp/f128 (bits ABI) against the host vectors in f128_vec.h; run() returns the number of wrong results.
 * wasm32: exported run(), called from node. riscv64: _start exits with min(fails, 255) through the exit ecall. */
typedef unsigned __int128 u128;
#include "f128_vec.h"
u128 sfb_addtf3(u128, u128), sfb_subtf3(u128, u128), sfb_multf3(u128, u128), sfb_divtf3(u128, u128);
static u128 j(v128 x) { return (u128)x.hi << 64 | x.lo; }
int run(void)
{
    int bad = 0;
    for (unsigned i = 0; i < sizeof V / sizeof V[0]; i++) {
        u128 a = j(V[i].a), b = j(V[i].b), r = V[i].op == 0 ? sfb_addtf3(a, b) : V[i].op == 1 ? sfb_subtf3(a, b) : V[i].op == 2 ? sfb_multf3(a, b) : sfb_divtf3(a, b);
        if (r != j(V[i].want)) bad++;
    }
    return bad;
}
#ifdef __riscv
void _start(void)
{
    int bad = run();
    register long a0 __asm__("a0") = bad > 255 ? 255 : bad, a7 __asm__("a7") = 93;
    __asm__ volatile("ecall" : : "r"(a0), "r"(a7));
    for (;;) {}
}
#endif
