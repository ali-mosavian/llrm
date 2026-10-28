// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// A value plus or minus a compare was `cmp; setcc; movzx; add|sub`, and `a == 0 ? -1 : 0` was `test; sete; movzx; neg`; the pin's
// combineAddOrSubToADCOrSBB makes them `cmp; adc|sbb` (ACTIONS3D, crc.c). `1 + (a == 0)` was `mov r, 1; cmp; adc r, 0`;
// combineADC makes it `ADC(0, 1)`: `xor r, r; cmp; adc r, 1`.
unsigned plus(unsigned a, unsigned b, unsigned x)
{
    return x + (a < b);
}
int one_plus(int a)
{
    return 1 + (a == 0);
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
// CHECK-LABEL: _one_plus@3 proc
// CHECK: xor {{[a-d]x}}, {{[a-d]x}}
// CHECK: cmp {{[a-d]x}}, 1
// CHECK: adc {{[a-d]x}}, 1
// CHECK: _one_plus@3 endp
// CHECK-LABEL: _mask@3 proc
// CHECK: cmp {{[a-d]x}}, 1
// CHECK: sbb
// CHECK-NOT: sete
// CHECK: _mask@3 endp
