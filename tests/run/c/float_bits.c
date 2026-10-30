// flags: -O2 -march=i486 | -O0 -march=i486 | -O2 -march=i486 -m32
/* A float read as an integer's bits and the other way round, whole and by halves: the bits go through memory because
   the x87 stack cannot hand them out. */
#include <stdint.h>
extern void report(long value);

union D { double d; uint32_t w[2]; };
union F { float f; uint32_t x; };

static uint32_t hi(double d) { union D u; u.d = d; return u.w[1]; }
static uint32_t lo(double d) { union D u; u.d = d; return u.w[0]; }
static double join(uint32_t high, uint32_t low) { union D u; u.w[1] = high; u.w[0] = low; return u.d; }
static uint32_t fbits(float f) { union F u; u.f = f; return u.x; }
static float ffrom(uint32_t x) { union F u; u.x = x; return u.f; }

int main(void)
{
    double d = 1.5;
    report((long)(hi(d) >> 12));
    report((long)lo(d));
    report((long)(hi(-2.0) >> 16));
    report((long)(join(0x40100000UL, 0) * 3.0));
    report((long)(fbits(1.0f) >> 8));
    report((long)(ffrom(0x41200000UL) * 2.0f));
    report((long)(ffrom(fbits(7.0f) ^ 0x80000000UL)));
    return 0;
}
