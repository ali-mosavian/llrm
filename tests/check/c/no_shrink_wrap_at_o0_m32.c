// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O0 -S -o /dev/stdout
// The pin runs ShrinkWrap only when optimizing, so at -O0 the registers a procedure keeps are saved at its entry. The
// pushes were set up late, in the block of the second loop, and popped back before the return. (The pin's own -O0 code
// for this body keeps none; the allocator here does, which is what puts the saves under test.)
static unsigned long values[8] = {0UL, 1UL, 2UL, 0x7FFFUL};
unsigned long sum(unsigned long a, unsigned long b, unsigned long c, unsigned long d)
{
    unsigned long total = 0, i;
    for (i = 0; i < 8; ++i)
        total += values[i] * 17UL;
    for (i = 0; i < 200; ++i)
        total = total * 31UL + (a ^ i) * (b + i) * (c - i) * (d | i) + (values[i & 7] * a + b) * (c ^ d) + (a << i) * (b >> i);
    return total;
}
// CHECK-LABEL: sum proc
// CHECK: push ebp
// CHECK-NEXT: mov ebp, esp
// CHECK-NEXT: sub esp
// CHECK-NEXT: push esi
// CHECK-NEXT: push edi
// CHECK-NEXT: push ebx
// CHECK-NEXT: L{{[0-9]+}}_1:
