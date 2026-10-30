#include "report.h"
/* #pragma aux inline code: registers in (parm), out (value) and changed (modify). The host build does the same in C. */
#ifdef __DOS__
#pragma aux add2 = "add al,bl" parm [al] [bl] value [al] modify exact [al]
char add2(char, char);
#pragma aux split parm [ax] [cx] value [dx ax] = "cwd" "idiv cx"
long split(int, int);
#pragma aux bump = "inc ax" 0x40 parm [ax] value [ax] modify exact [ax]
int bump(int);
#else
static char add2(char a, char b) { return (char)(a + b); }
static long split(int a, int b) { return ((long)(a % b) << 16) | (unsigned short)(a / b); }
static int bump(int a) { return a + 2; }
#endif
int main(void)
{
    int i;
    long sum = 0;
    report(add2(100, 27));
    report(add2(-5, 3));
    report(split(1000, 7));
    report(split(-1000, 7));
    for (i = 0; i < 10; i++) sum += bump(i) + add2((char)i, 1);
    report(sum);
    return 0;
}
