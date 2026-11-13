/* A guard whose flags come from a compare chain in front of the branch, in a loop with a long step: a group of
 * instructions moved onto the continuing edge must not take the compare that feeds the branch with it (the guard
 * `if (x & low)` was tested before its AND ran: until_bit(1, 15) never returned). */
#include <stdint.h>
extern void report(long value);

__attribute__((noinline)) uint32_t until_bit(uint32_t x, uint32_t pos)
{
    uint32_t m = (uint32_t)1 << pos;
    while (!(x & m))
        x <<= 1;
    return x;
}

int main(void)
{
    static const uint32_t xs[] = {1u, 2u, 0x100u, 0xff00u, 0xffffffffu};
    static const uint32_t positions[] = {0u, 3u, 8u, 15u, 31u};
    for (unsigned i = 0; i < sizeof xs / sizeof xs[0]; i++) {
        volatile uint32_t x = xs[i];
        for (unsigned j = 0; j < sizeof positions / sizeof positions[0]; j++) {
            uint32_t pos = positions[j];
            uint32_t low = pos == 31 ? ~(uint32_t)0 : ((uint32_t)2 << pos) - 1u;
            if (x & low)
                report((int32_t)until_bit(x, pos));
        }
    }
    return 0;
}
