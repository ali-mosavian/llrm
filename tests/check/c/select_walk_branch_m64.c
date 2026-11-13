// RUN: llrm-cc %s -m64 {-Os | -O2 | -O3} -S -o /dev/stdout
// A select of two constants on a loop-carried load-address chain is a branch at -O2/-O3 (DEVIATE SELOPT-C): bintree -O3 was
// `setg; sub` between two dependent loads, 18.0k clk, and 10.0k as a branch. -Os keeps the arithmetic.
// CHECK-LABEL: walk proc
// CHECK-NOT[-O2]: set{{[gl]}}
// CHECK-NOT[-O3]: set{{[gl]}}
// CHECK[-Os]: set{{[gl]}}
int walk(int *t, int key, int node)
{
    int at = 1;
    for (;;) {
        int slot = 3 * at + (key < t[3 * at] ? 1 : 2);
        if (!t[slot]) {
            t[slot] = node;
            return slot;
        }
        at = t[slot];
    }
}
