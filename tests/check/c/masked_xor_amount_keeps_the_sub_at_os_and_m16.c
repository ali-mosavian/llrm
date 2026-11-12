// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 -S -o /dev/stdout
// RUN: llrm-cc %s -m16 -O2 -march=i386 -S -o /dev/stdout
// `26 - (n ^ 31)` is the pin's combineSub fold `or n, -32; add n, 27` (X86ISelLowering.cpp#L60933; the pin's llc gives
// `orb $-32; addb $27` on the same IR). No `mov 26; sub`, no `lea`: -Os has no byte `lea`, m16 no three-address add.
// Folding it to `n - 5` earlier (SD448, an InstCombine rewrite) made 18 m16, 18 z80 and 11 m32 -Os cells larger.
unsigned amount(unsigned a, unsigned x)
{
    unsigned k = a & 31;
    return (x << (26 - (k ^ 31))) + k;
}

// CHECK-LABEL: amount{{.*}} proc
// CHECK: or {{[a-d]l}}, -32
// CHECK-NEXT: add {{[a-d]l}}, 27
// CHECK-NOT: lea
