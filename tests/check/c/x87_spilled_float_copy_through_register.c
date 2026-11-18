// RUN: llrm-cc %s -O2 -w -march=i386 -S -o /dev/stdout
// A float copied from one frame cell to another by `fld dword [a]; fstp dword [b]` is 10 clocks on a 486; two dword moves through a
// free register are 2 (x87rotate m16 -O2: 1709 clocks for 1559). Needs a register the control-word restore does not make live.
long rot(short n, float seed)
{
    float a0 = seed, a1 = seed + 1, a2 = seed + 2, a3 = seed + 3, a4 = seed + 4;
    float a5 = seed + 5, a6 = seed + 6, a7 = seed + 7, a8 = seed + 8, a9 = seed + 9;
    float t;
    short i;
    for (i = 0; i < n; ++i) {
        t = a0;
        a0 = a1; a1 = a2; a2 = a3; a3 = a4; a4 = a5;
        a5 = a6; a6 = a7; a7 = a8; a8 = a9; a9 = t;
        a0 = a0 + a9 * 0.5f;
    }
    return (long)(a0 + 2 * a1 + 3 * a2 + 4 * a3 + 5 * a4 + 6 * a5 + 7 * a6 + 8 * a7 + 9 * a8 + 10 * a9);
}
// The slots are the allocator's to number (#9367 moved them): the symptom is a copy through ecx, twice in a row.
// CHECK: L0_3:
// CHECK: mov ecx, dword ptr [bp-{{[0-9]+}}]
// CHECK-NEXT: mov dword ptr [bp-{{[0-9]+}}], ecx
// CHECK-NEXT: mov ecx, dword ptr [bp-{{[0-9]+}}]
// CHECK-NEXT: mov dword ptr [bp-{{[0-9]+}}], ecx
