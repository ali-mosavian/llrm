// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
/* The OS layer's memory request: two grants are not null, writable, and do not overlap. */
#include "llrm_os.h"
extern void report(long value);
int main(void)
{
    unsigned char *first = llrm_os_more(64);
    unsigned char *second = llrm_os_more(64);
    unsigned sum = 0;
    int i;
    report(first != 0);
    report(second != 0);
    for (i = 0; i < 64; i++) first[i] = 1;
    for (i = 0; i < 64; i++) second[i] = 2;
    for (i = 0; i < 64; i++) sum += first[i];
    report(sum);
    return 0;
}
