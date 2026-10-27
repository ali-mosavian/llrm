#include "report.h"
/* A huge region past 64K, indexed by a long, stepped by a pointer, subtracted: 100000 bytes of free memory at 6000:0000. */
#ifdef __DOS__
#define REGION ((unsigned char huge *)0x60000000L)
#else
static unsigned char store[100000];
#define REGION store
#endif
int main(void)
{
    unsigned char huge *buf = REGION;
    unsigned char huge *p;
    long i, sum = 0;
    for (i = 0; i < 100000L; i++) buf[i] = (unsigned char)(i * 7);
    for (i = 0; i < 100000L; i++) sum += buf[i];
    report(sum);
    p = buf + 70000L;
    report(p - buf);
    report(p[5]);
    report(buf[99999L]);
    for (p = buf; p < buf + 100000L; p++) sum -= *p;
    report(sum);
    return 0;
}
