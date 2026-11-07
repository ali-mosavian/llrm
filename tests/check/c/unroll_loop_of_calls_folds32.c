// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 -S -o /dev/stdout
// A 7-trip loop of calls whose argument is a switch over the counter is seven `push imm; call`. The pin gives up on a call, and the priced unroll took the copies for the rolled body (305 B priced for 61), vetoed the early unroll, and left the switch's lookup table dead in the data after the late one (static_pointers +6 B).
extern void report(long value);
static int pick(int i, int a)
{
    switch (i) {
    case 0: return a + 1;
    case 1: return a + 2;
    case 2: return a * 3;
    case 3: return a - 4;
    case 4: return a << 2;
    case 5: return a ^ 7;
    default: return -1;
    }
}
void seven(void)
{
    int i;
    for (i = 0; i < 7; i++)
        report(pick(i, 10));
}
// CHECK-NOT: switch$table
// CHECK: _seven proc
// CHECK: _seven endp
// CHECK-NOT: switch$table
