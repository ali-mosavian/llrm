#include "report.h"
/* A static huge object past 64K: laid down as consecutive 64K segments, stepped through by a huge pointer. */
#ifdef __DOS__
#define HUGE huge
#else
#define HUGE
#endif
static unsigned char HUGE big[100000] = { 1, 2, 3 };
static unsigned char HUGE tail[70000];
int main(void)
{
    long i, sum = 0;
    unsigned char HUGE *p;
    for (i = 3; i < 100000L; i++) big[i] = (unsigned char)(i * 7);
    for (p = big; p < big + 100000L; p++) sum += *p;
    report(sum);
    report(big[65535]);
    report(big[65536]);
    report(big[99999]);
    tail[69999] = 9;
    report(tail[69999] + tail[65536]);
    return 0;
}
