// RUN: llrm-c %s -m16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// A value plus or minus a compare was `cmp; setcc; movzx; add|sub`, and `a == 0 ? -1 : 0` was `test; sete; movzx; neg`; the pin's
// combineAddOrSubToADCOrSBB makes them `cmp; adc|sbb` (ACTIONS3D, crc.c).
unsigned plus(unsigned a, unsigned b, unsigned x)
{
    return x + (a < b);
}
int mask(int a)
{
    return a == 0 ? -1 : 0;
}
// CHECK-LABEL: _plus@3 proc
// CHECK: cmp
// CHECK: adc
// CHECK-NOT: setb
// CHECK: _plus@3 endp
// CHECK-LABEL: _mask@3 proc
// CHECK: cmp {{[a-d]x}}, 1
// CHECK: sbb
// CHECK-NOT: sete
// CHECK: _mask@3 endp
