// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -O1 -S -o /dev/stdout
// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=pentium -O1 -S -o /dev/stdout
// A one-slot frame is `push eax` / `pop ecx` where that is no slower than `sub esp, 4` / `add esp, 4`: -Os (4 B shorter), and the
// Pentium (pop 1 clk). The 486 pops in 4 clocks: sfm16_dbg -O1 ran 12,527 -> 12,530 clk against main, 3 more per call, for 4 B.
// CHECK-LABEL: _f proc
// CHECK-NOT[i486 -O1]: push eax
// CHECK[i486 -O1]: sub esp, 4
// CHECK-NOT[i486 -O1]: pop ecx
// CHECK[i486 -O1]: add esp, 4
// CHECK-NOT[i486 -O2]: push eax
// CHECK[i486 -O2]: sub esp, 4
// CHECK-NOT[i486 -O2]: pop ecx
// CHECK[i486 -O2]: add esp, 4
// CHECK[-Os]: push eax
// CHECK[-Os]: pop ecx
// CHECK[pentium]: push eax
// CHECK[pentium]: pop ecx
int f(int n)
{
    volatile int t = n;
    return t + 1;
}
