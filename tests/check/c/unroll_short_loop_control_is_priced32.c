// RUN: llrm-cc %s -m32 -mabi=sysv -O3 -march=i486 -S -o /dev/stdout
// A 3-trip loop of four stores: the priced unroll (UP1) counted its control (compare, counter step, taken `jne`) at the body's average
// clocks, 6 clocks saved against 35 bytes added, and kept the loop; it runs 25 clocks (bench lru -O3: 165 -> 140 clocks, +22 bytes).
short bord[3], bprev[3], bnext[3], bstamp[3];
void clear3(void)
{
    short index;
    for (index = 0; index < 3; ++index) {
        bord[index] = 0;
        bprev[index] = -1;
        bnext[index] = -1;
        bstamp[index] = 0;
    }
}
// CHECK-LABEL: _clear3 proc
// CHECK-NOT: jne
// CHECK-NOT: jl
// CHECK: _clear3 endp
