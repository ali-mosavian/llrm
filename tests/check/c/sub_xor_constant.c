// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// X86 combineSub (X86ISelLowering.cpp#L60916): `sub C1, (xor X, C2)` is `add (xor X, ~C2), C1 + 1`, so x86, which cannot encode
// an immediate left side of a sub, spends no register on 26: `26 - ((n & 31) ^ 31)` is the pin's `or eax, -32; add eax, 27`.
// DEVIATE SD448 made it `and eax, 31; add eax, -5` (the same two instructions) only where a known-bits test held.
unsigned f(unsigned n) { return 26u - ((n & 31u) ^ 31u); }
// CHECK: or eax, -32
// CHECK-NEXT: add eax, 27
// CHECK-NEXT: ret
