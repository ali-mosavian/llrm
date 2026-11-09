// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// combineSub's first fold: `sub C1, (xor X, C2)` is `add (xor X, ~C2), C1+1`, one register less (the pin, X86ISelLowering.cpp#L60933).
// It was `mov eax, 100; sub eax, ecx`. SD448 takes the low-mask shapes first.
unsigned f(unsigned n) { return 100u - (n ^ 0xF0u); }
// CHECK-LABEL: _f proc
// CHECK: mov eax, -241
// CHECK-NEXT: xor eax, dword ptr [esp+4]
// CHECK-NEXT: add eax, 101
// CHECK-NEXT: ret
