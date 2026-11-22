#include <stdint.h>
#include <stdio.h>

static int64_t rint64(double x)
{
    int64_t v;
    __asm__("cvtsd2si %1, %0" : "=r"(v) : "x"(x));
    return v;
}

static int32_t rint32(float x)
{
    int32_t v;
    __asm__("cvtss2si %1, %0" : "=r"(v) : "x"(x));
    return v;
}

static uint32_t swap32(uint32_t x)
{
    uint32_t y;
    __asm__("bswap %0" : "=r"(y) : "0"(x));
    return y;
}

/* three inputs, one output: four distinct registers */
static uint64_t add3(uint64_t a, uint64_t b, uint64_t c)
{
    uint64_t s;
    __asm__("lea (%1,%2), %0\n\tadd %3, %0" : "=r"(s) : "r"(a), "r"(b), "r"(c));
    return s;
}

/* rcx is clobbered by the block and must not be an operand; a value in it stays live across */
static uint64_t clobbers_rcx(uint64_t a, uint64_t keep)
{
    uint64_t s;
    __asm__("mov %1, %0\n\txor %%rcx, %%rcx" : "=r"(s) : "r"(a) : "rcx");
    return s + keep;
}

/* doubles live across the block, in the registers the pool may pick */
static double live_vectors(double a, double b, double c)
{
    const double t = a * b;
    const double u = a + c;
    const int64_t v = rint64(c);
    return t + u + (double)v + a + b + c;
}

static uint64_t twice(uint64_t x)
{
    uint64_t one, two;
    __asm__("mov %1, %0\n\tadd $1, %0" : "=r"(one) : "r"(x));
    __asm__("mov %1, %0\n\tadd $2, %0" : "=r"(two) : "r"(one));
    return one * 1000 + two;
}

int main(void)
{
    volatile double d25 = 2.5, d35 = 3.5, a = 1.5, b = 2.0;
    volatile float f = -2.5f;
    volatile uint32_t w = 0x11223344u;
    volatile uint64_t seven = 7, one = 1, two = 2, three = 3;
    int bad = 0;
    bad += rint64(d25) != 2;
    bad += rint64(d35) != 4;
    bad += rint32(f) != -2;
    bad += swap32(w) != 0x44332211u;
    bad += add3(one, two, three) != 6;
    bad += clobbers_rcx(seven, three * 5) != 22;
    bad += live_vectors(a, b, d25) != 3.0 + 4.0 + 2.0 + 1.5 + 2.0 + 2.5;
    bad += twice(seven) != 8 * 1000 + 10;
    uint64_t sum = 0;
    for (uint64_t i = 0; i < 10; i++)
        sum += add3(i, one, two) + clobbers_rcx(i, i);
    bad += sum != 45 + 30 + 45 + 45;
    printf("bad %d\n", bad);
    return 0;
}
