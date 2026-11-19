// RUN: llrm-cc %s --target=x86_32-linux -m32 -mabi=sysv -O2 -march=i486 --isel=dag -S -o /dev/stdout
// x_ld_sum at m32 -O2 (FF2): `x[r] += 1` was `fld1; faddp`, 12 clocks and 4 bytes, where `fadd dword [K]` is 8 clocks and 6 bytes. At speed the memory operand wins. The x86_32-linux target: its `long double` is the 80-bit form the fold prices; the default -m32 target (wat) makes it a double.
// CHECK-LABEL: bump proc
// CHECK: fadd dword ptr
// CHECK-NOT: fld1
// CHECK: bump endp
extern long double x[8];
void bump(int r)
{
    x[r] += 1;
}
