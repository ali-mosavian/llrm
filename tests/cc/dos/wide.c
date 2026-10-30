#include "report.h"
static unsigned long long mix(unsigned long long x) { x ^= x >> 33; x *= 0xff51afd7ed558ccdULL; x ^= x >> 29; return x; }
int main(void)
{
    unsigned long long h = 1;
    for (int i = 0; i < 100; i++) h = mix(h + i);
    report((long)(h >> 40));
    report((long)(h & 0x7fffffff));
    return 0;
}
