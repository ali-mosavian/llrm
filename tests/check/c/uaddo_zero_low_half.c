// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// `sign + (a << 32) + b` at i486 was `xor eax, eax; add; adc; add; adc edx, 0` (6 insns): nothing folded `uaddo x, 0` or
// `uaddo_carry (add X, Y), 0, c` (the pin's visitADDO, visitUADDO_CARRY); it is `add; adc` (2 insns) as in the pin.
unsigned long long f(unsigned long long sign, unsigned a, unsigned b)
{
    return sign + (((unsigned long long)a) << 32) + (unsigned long long)b;
}
// CHECK-LABEL: f proc
// CHECK-NOT: xor eax, eax
// CHECK: add eax
// CHECK-NEXT: mov edx
// CHECK-NEXT: adc edx
// CHECK-NOT: adc edx, 0
