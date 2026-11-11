// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 -S -o /dev/stdout
// RUN: llrm-cc %s -m16 -O2 -march=i386 -S -o /dev/stdout
// `26 - (n ^ 31)` stays `mov cl, 26; sub cl, al` where the add form is a byte longer: -Os (no byte `lea`) and m16 (no three-address add).
// The fold to `n - 5` (SD448) made 18 m16 and 18 z80 cells and 11 m32 -Os cells larger when it was an InstCombine rewrite.
unsigned amount(unsigned a, unsigned x)
{
    unsigned k = a & 31;
    return (x << (26 - (k ^ 31))) + k;
}

// CHECK-LABEL: amount{{.*}} proc
// CHECK: mov cl, 26
// CHECK-NEXT: sub cl,
// CHECK-NOT: lea
