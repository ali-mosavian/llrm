// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O0 -S -o /dev/stdout
// The pin runs ShrinkWrap only when optimizing, so at -O0 the registers a procedure keeps are saved at its entry. The
// `push esi` was set up late, in the block of the second loop, and `pop esi` came back before the return.
static unsigned long values[8] = {0UL, 1UL, 2UL, 0x7FFFUL};
unsigned long sum(void)
{
    unsigned long total = 0, i, x = 0x9E3779B9UL;
    for (i = 0; i < 8; ++i)
        total += values[i] * 17UL;
    for (i = 0; i < 200; ++i) {
        x = x * 1664525UL + 1013904223UL;
        total = total * 31UL + x * 17UL;
    }
    return total;
}
// CHECK-LABEL: sum proc
// CHECK: push ebp
// CHECK-NEXT: mov ebp, esp
// CHECK-NEXT: sub esp
// CHECK-NEXT: push esi
// CHECK-NEXT: L{{[0-9]+}}_1:
