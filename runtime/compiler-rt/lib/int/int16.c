/* 16-bit multiply, divide and shifts for CPUs without the instruction (Z80). Division by zero: quotient all ones,
 * remainder the dividend; INT_MIN / -1 is INT_MIN, remainder 0. A shift count is taken as given: at or over 16 is undefined in C. */
#include "sf_int.h"

uint16_t SFB_NAME(mulhi3)(uint16_t a, uint16_t b)
{
    uint16_t r = 0;
    for (; b; b >>= 1, a <<= 1)
        if (b & 1) r += a;
    return r;
}

/* Shift-subtract: the divisor is aligned under the dividend's top bit, one quotient bit per step. */
static uint16_t udivmod(uint16_t n, uint16_t d, uint16_t *r)
{
    uint16_t q = 0;
    if (!d) { *r = n; return 0xFFFFu; }
    if (d > n) { *r = n; return 0; }
    int s = 0;
    while (!(d & 0x8000u) && (uint16_t)(d << 1) <= n) { d <<= 1; s++; }
    for (; s >= 0; s--) {
        q <<= 1;
        if (n >= d) { n -= d; q |= 1; }
        d >>= 1;
    }
    *r = n;
    return q;
}

uint16_t SFB_NAME(udivhi3)(uint16_t n, uint16_t d) { uint16_t r; return udivmod(n, d, &r); }
uint16_t SFB_NAME(umodhi3)(uint16_t n, uint16_t d) { uint16_t r; udivmod(n, d, &r); return r; }

int16_t SFB_NAME(divhi3)(int16_t n, int16_t d)
{
    uint16_t r, q = udivmod(n < 0 ? 0u - (uint16_t)n : (uint16_t)n, d < 0 ? 0u - (uint16_t)d : (uint16_t)d, &r);
    return (int16_t)((n < 0) != (d < 0) && d ? 0u - q : q);
}

int16_t SFB_NAME(modhi3)(int16_t n, int16_t d)
{
    uint16_t r;
    udivmod(n < 0 ? 0u - (uint16_t)n : (uint16_t)n, d < 0 ? 0u - (uint16_t)d : (uint16_t)d, &r);
    return (int16_t)(n < 0 ? 0u - r : r);
}

uint16_t SFB_NAME(ashlhi3)(uint16_t a, int n) { while (n-- > 0) a <<= 1; return a; }
uint16_t SFB_NAME(lshrhi3)(uint16_t a, int n) { while (n-- > 0) a >>= 1; return a; }
int16_t SFB_NAME(ashrhi3)(int16_t a, int n) { while (n-- > 0) a = (int16_t)(a >> 1); return a; }
