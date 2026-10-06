// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
/* The OS layer's exit ends the program at once, with the output before it written and none after it. */
#include "llrm_os.h"
extern void report(long value);
int main(void)
{
    report(1);
    llrm_os_exit(0);
    report(2);
    return 1;
}
