/* Freestanding driver: lib/fp f32 and f64 multiply against the host vectors in fp_mul_vec.h; run() returns the number of wrong results.
 * wasm32: exported run(), called from node. riscv64/aarch64: _start exits with min(fails, 255) through the exit system call. */
#include "fp_mul_vec.h"
float sfb_mulsf3(float, float);
double sfb_muldf3(double, double);
int run(void)
{
    int bad = 0;
    for (unsigned i = 0; i < sizeof V / sizeof V[0]; i++) {
        if (V[i].op == 0) {
            union { unsigned u; float f; } a = { (unsigned)V[i].a }, b = { (unsigned)V[i].b }, r;
            r.f = sfb_mulsf3(a.f, b.f);
            if (r.u != (unsigned)V[i].want) bad++;
        } else {
            union { unsigned long long u; double f; } a = { V[i].a }, b = { V[i].b }, r;
            r.f = sfb_muldf3(a.f, b.f);
            if (r.u != V[i].want) bad++;
        }
    }
    return bad;
}
#if defined(__riscv) || defined(__aarch64__)
void _start(void)
{
    int bad = run();
    long code = bad > 255 ? 255 : bad;
#ifdef __riscv
    register long a0 __asm__("a0") = code, a7 __asm__("a7") = 93;
    __asm__ volatile("ecall" : : "r"(a0), "r"(a7));
#else
    register long x0 __asm__("x0") = code, x8 __asm__("x8") = 93;
    __asm__ volatile("svc 0" : : "r"(x0), "r"(x8));
#endif
    for (;;) {}
}
#endif
