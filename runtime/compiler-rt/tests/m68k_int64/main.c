/* uint64 operations compiled by the target compiler vs Python: counts per op, all must be 0. See gen.py. */
#include <stdint.h>
#include "v.h"
uint64_t sf_o_shl(uint64_t, int), sf_o_shr(uint64_t, int), sf_o_add(uint64_t, uint64_t), sf_o_sub(uint64_t, uint64_t), sf_o_mul(uint64_t, uint64_t), sf_o_mulw(uint32_t, uint32_t), sf_o_div(uint64_t, uint64_t), sf_o_mod(uint64_t, uint64_t), sf_o_sticky(uint64_t, int);
int sf_o_lt(uint64_t, uint64_t);
static long sys3(long n, long a, long b, long c) { register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c; __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory"); return d0; }
static void say(const char *s) { int n = 0; while (s[n]) n++; sys3(4, 1, (long)s, n); }
static void dec(long v) { char b[12]; int i = 11; b[i] = 0; b[--i] = ' '; do { b[--i] = '0' + v % 10; v /= 10; } while (v); say(b + i); }
#define W(k) (((uint64_t)v[2 * (k)] << 32) | v[2 * (k) + 1])
int main(void) {
    static const char *const nm[] = {"shl", "shr", "add", "sub", "mul", "mulw", "div", "mod", "sticky", "lt"};
    long bad[10] = {0};
    for (int i = 0; i < NV; i++) {
        const uint32_t *v = V[i]; uint64_t x = W(0), y = W(1); int n = v[22];
        bad[0] += sf_o_shl(x, n) != W(2); bad[1] += sf_o_shr(x, n) != W(3); bad[2] += sf_o_add(x, y) != W(4); bad[3] += sf_o_sub(x, y) != W(5);
        bad[4] += sf_o_mul(x, y) != W(6); bad[5] += sf_o_mulw((uint32_t)x, (uint32_t)y) != W(7); bad[6] += sf_o_div(x, y) != W(8); bad[7] += sf_o_mod(x, y) != W(9);
        bad[8] += sf_o_sticky(x, n) != W(10); bad[9] += sf_o_lt(x, y) != (int)v[23];
    }
    for (int k = 0; k < 10; k++) { say(nm[k]); say("="); dec(bad[k]); }
    say("\n"); sys3(1, 0, 0, 0); return 0;
}
void _start(void) { main(); for (;;) ; }
