// flags: -O2 -march=i486
// targets: x86-m16
/* The OS layer's block allocation (real mode): a block outside the program's own, far from its data segment, writable
   whole, given back and taken again. Without trimming the program's block first, DOS had no paragraph to give. */
#include "llrm_os.h"
extern void report(long value);
int main(void)
{
    unsigned first = llrm_os_block_alloc(3000);
    unsigned second;
    unsigned char __far *p;
    unsigned long sum = 0;
    unsigned i;
    report(first != 0);
    p = (unsigned char __far *)((unsigned long)first << 16);
    for (i = 0; i < 40000u; i++) p[i] = 2;
    for (i = 0; i < 40000u; i++) sum += p[i];
    report((long)sum);
    llrm_os_block_free(first);
    second = llrm_os_block_alloc(3000);
    report(second != 0);
    llrm_os_block_free(second);
    return 0;
}
