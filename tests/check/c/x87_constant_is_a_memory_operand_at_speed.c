// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 --isel=dag -S -o /dev/stdout
// x_ld_sum at m32 -O2 (FF2): `x[r] += 1` was `fld1; faddp`, 12 clocks and 4 bytes, where `fadd dword [K]` is 8 clocks and 6 bytes. At speed the memory operand wins.
// CHECK-LABEL: _bump proc
// CHECK: fadd dword ptr
// CHECK-NOT: fld1
// CHECK: _bump endp
extern long double x[8];
void bump(int r)
{
    x[r] += 1;
}
