// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
/* The OS layer's text screen is the colour text memory: a cell written reads back, and is put back. */
#include "llrm_os.h"
extern void report(long value);
int main(void)
{
    volatile unsigned char *screen = LLRM_OS_TEXT_SCREEN;
    unsigned char saved = screen[0];
    screen[0] = 65;
    report(screen[0]);
    screen[0] = saved;
    report(screen[0] == saved);
    return 0;
}
