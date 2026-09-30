#include "debug.h"
struct pt { short x; long y; };
union mix { unsigned char b; unsigned short w; };
unsigned long gul = 4000000000ul;
struct pt gp;
union mix gu;
int ga[10];
char far *gfp;
int huge *ghp;

long f(short a, struct pt *b, char c)
{
    int l = a;
    static int st;
    st++;
    return l + b->y + c + st + twice(l);
}

void v(void)
{
}
