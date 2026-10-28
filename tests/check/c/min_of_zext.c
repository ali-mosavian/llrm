// RUN: llrm-cc %s -m16 -Os -march=i386 -S -o /dev/stdout
// The closed form of this sum took umin.i33 of a zext, so a 64-bit compare, an `and` of a zero-extended value with 0xFFFFFFFF
// and an `and`/`cmp`/`sbb` chain whose result `mov` overwrote: 107 bytes at m16 -Os where the narrow umin is 16-bit.
long b(unsigned short n)
{
    long s = 0;
    unsigned short i;
    for (i = 0; i < n && i < 10; i++) s += i;
    return s;
}
// CHECK-LABEL: _b
// CHECK-NOT: sbb
// CHECK-NOT: 4294967295
// CHECK: endp
