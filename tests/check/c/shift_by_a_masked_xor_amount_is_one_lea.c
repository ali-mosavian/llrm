// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 -S -o /dev/stdout
// `26 - (n ^ 31)` with `n <= 31` stayed a `sub` (`mov cl, 26; sub cl, al`). With `n` live after it, the byte add that replaces it was
// `mov ecx, eax; add cl, -5` (five bytes): it is `lea ecx, [eax - 5]` (three), as in gcc. `__floatsisf` -O2 385 -> 382 B.
unsigned amount(unsigned a, unsigned x)
{
    unsigned k = a & 31;
    return (x << (26 - (k ^ 31))) + k;
}

// CHECK-LABEL: _amount proc
// CHECK: lea ecx, [{{e[a-d]x}}-5]
// CHECK-NOT: add cl
// CHECK-NOT: sub cl
