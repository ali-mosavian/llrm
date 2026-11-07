/* f64 divide driver: edge pairs, then random normal ones (xorshift); the xor of the quotients. HOST builds it for the host FPU. */
typedef unsigned long long u64;
#ifndef HOST
u64 __divdf3(u64, u64);
#define DIV(a, b) __divdf3(a, b)
#else
#include <stdio.h>
#include <string.h>
static u64 hdiv(u64 a, u64 b) { double x, y, z; u64 r; memcpy(&x, &a, 8); memcpy(&y, &b, 8); z = x / y; memcpy(&r, &z, 8); return r; }
#define DIV(a, b) hdiv(a, b)
#endif
#ifdef NOEDGE
#define NV 32
#define NEDGE 0
#else
#define NV 24
#define NEDGE (sizeof edge / sizeof edge[0])
#endif
static const u64 edge[][2] = {
    {0x3FF0000000000000ull, 0x3FF0000000000000ull}, /* 1/1 */
    {0x3FF0000000000000ull, 0x4008000000000000ull}, /* 1/3 */
    {0x4008000000000000ull, 0x3FF0000000000000ull}, /* 3/1 */
    {0x0000000000000000ull, 0x3FF0000000000000ull}, /* 0/1 */
    {0x3FF0000000000000ull, 0x0000000000000000ull}, /* 1/0 */
    {0x7FEFFFFFFFFFFFFFull, 0x0010000000000000ull}, /* max/min normal: overflow */
    {0x0010000000000000ull, 0x7FEFFFFFFFFFFFFFull}, /* min/max: underflow */
    {0x0010000000000000ull, 0x4008000000000000ull}, /* min normal / 3: denormal quotient */
    {0x0010000000000001ull, 0x4000000000000000ull}, /* denormal quotient, exact */
    {0x3FFFFFFFFFFFFFFFull, 0x3FF0000000000001ull}, /* near 2 / near 1 */
    {0x3FF0000000000001ull, 0x3FFFFFFFFFFFFFFFull}, /* near 1 / near 2 */
    {0x3FF8000000000000ull, 0x3FF8000000000000ull}, /* a/a */
    {0xBFF0000000000000ull, 0x4008000000000000ull}, /* -1/3 */
    {0x3FF0000000000000ull, 0xC008000000000000ull}, /* 1/-3 */
    {0x000FFFFFFFFFFFFFull, 0x3FF0000000000000ull}, /* denormal / 1 */
    {0x3FF0000000000000ull, 0x000FFFFFFFFFFFFFull}, /* 1 / denormal: overflow */
};
int main(void) {
    u64 s = 0x9E3779B97F4A7C15ull, acc = 0, a, b; unsigned i;
    for (i = 0; i < NEDGE; i++) acc ^= DIV(edge[i][0], edge[i][1]);
    for (i = 0; i < NV - NEDGE; i++) {
        s ^= s << 13; s ^= s >> 7; s ^= s << 17; a = (s & 0x800FFFFFFFFFFFFFull) | ((u64)(1010 + ((s >> 52) & 0x1F)) << 52);
        s ^= s << 13; s ^= s >> 7; s ^= s << 17; b = (s & 0x800FFFFFFFFFFFFFull) | ((u64)(1010 + ((s >> 52) & 0x1F)) << 52);
        acc ^= DIV(a, b);
    }
#ifdef HOST
    printf("%016llx\n", acc);
#else
    *(volatile u64 *)0x8f00 = acc;
#endif
    return 0;
}
