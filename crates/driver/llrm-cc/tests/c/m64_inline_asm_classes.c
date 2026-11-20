#include <stdint.h>

int64_t rint64(double x)
{
    int64_t v;
    __asm__("cvtsd2si %1, %0" : "=r"(v) : "x"(x));
    return v;
}

int32_t rint32(float x)
{
    int32_t v;
    __asm__("cvtss2si %1, %0" : "=r"(v) : "x"(x));
    return v;
}

uint32_t swap32(uint32_t x)
{
    uint32_t y;
    __asm__("bswap %0" : "=r"(y) : "0"(x));
    return y;
}
