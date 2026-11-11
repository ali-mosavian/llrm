// RUN: llrm-cc %s -m64 -fno-pic -O2 -S -o /dev/stdout
// At -fno-pic the pin allows a global base with an index, so the load's address is free: this loop is size 8 and the partial unroll takes 4 (28-2)/(8-2). The Tti forbade it at m64 always, priced the address 1, took size 9 and unrolled by 2: scroll -O2 +2.9% clocks to clang.
// CHECK-LABEL: ck proc
// CHECK: imul
// CHECK: imul
// CHECK: imul
// CHECK: imul
// CHECK: ck endp
short back[2000];
long ck(void)
{
    long total = 0;
    short i;

    for (i = 0; i < 2000; ++i) total += (long)back[i] * ((i & 15) + 1);
    return total;
}
