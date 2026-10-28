// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// `sext i1 (setcc)` was promoted to `shl ax, 15; sar ax, 15` (QuickBASIC's -1/0 booleans: ACTIONS3D +46 sar, +41 shl over the old selector). The pin builds
// SIGN_EXTEND_INREG, which LegalizeDAG expands to `0 - (x & 1)`, and the setcc's known bits drop the `and`: `movzx ax, al; neg ax`.
int f(int a)
{
    return a < 0x1234 ? -1 : 0;
}
// CHECK-LABEL: _f@3 proc
// CHECK-NOT: sar
// CHECK: neg
// CHECK: _f@3 endp
