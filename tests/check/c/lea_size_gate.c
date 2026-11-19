// RUN: llrm-cc %s -m16 -mabi=ia16 -march=i486 -Os -w --isel=dag -S -o /dev/stdout
// At m16 -Os `x + y + 1` came out as `lea eax,[eax+edx+1]`, 6 B (67 66 8D 44 10 01); the dying source needs no
// copy, so `add eax,edx` + `inc eax` is 5 B. The two-address conversion made a lea a byte longer than the add.
// Which of ecx/edx holds the second operand is the allocator's order, not what this gate is about.
// CHECK-LABEL: {{_f(@3)?}} proc
// CHECK-NOT: lea
// CHECK: add eax, {{e[cd]x}}
// CHECK: inc eax
long f(long x, long y)
{
    return x + y + 1;
}
