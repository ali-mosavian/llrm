/* Freestanding driver: lib/fp f32 and f64 multiply, divide, int-to-float against the host vectors in fp_mul_vec.h; run() returns the number of wrong results.
 * wasm32: exported run(), called from node. riscv64/aarch64: _start exits with min(fails, 255) through the exit system call. */
#include "fp_mul_vec.h"
float sfb_mulsf3(float, float);
double sfb_muldf3(double, double);
float sfb_divsf3(float, float);
double sfb_divdf3(double, double);
float sfb_floatsisf(int);
double sfb_floatsidf(int);
double sfb_floatdidf(long long);
double sfb_floatunsidf(unsigned);
double sfb_floatundidf(unsigned long long);
#ifndef REPS
#define REPS 1
#endif
int run(void)
{
    int bad = 0;
    for (int rep = 0; rep < REPS; rep++)
    for (unsigned i = 0; i < sizeof V / sizeof V[0]; i++) {
        int op = V[i].op;
        union { unsigned long long u; double f; } a = { V[i].a }, b = { V[i].b }, r;
        union { unsigned u; float f; } fa = { (unsigned)V[i].a }, fb = { (unsigned)V[i].b }, fr;
        switch (op) {
        case 0: fr.f = sfb_mulsf3(fa.f, fb.f); break;
        case 2: fr.f = sfb_divsf3(fa.f, fb.f); break;
        case 4: fr.f = sfb_floatsisf((int)V[i].a); break;
        case 1: r.f = sfb_muldf3(a.f, b.f); break;
        case 3: r.f = sfb_divdf3(a.f, b.f); break;
        case 5: r.f = sfb_floatsidf((int)V[i].a); break;
        case 6: r.f = sfb_floatdidf((long long)V[i].a); break;
        case 7: r.f = sfb_floatunsidf((unsigned)V[i].a); break;
        default: r.f = sfb_floatundidf(V[i].a); break;
        }
        if (op == 0 || op == 2 || op == 4) bad += fr.u != (unsigned)V[i].want;
        else bad += r.u != V[i].want;
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
