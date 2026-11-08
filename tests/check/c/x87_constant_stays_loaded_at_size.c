// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 --isel=dag -S -o /dev/stdout
// FF2 at -Os m32: `fld1; faddp` is 4 bytes, `fadd dword [K]` 6. Built for size the constant stays loaded.
// CHECK-LABEL: _bump proc
// CHECK: fld1
// CHECK: _bump endp
extern long double x[8];
void bump(int r)
{
    x[r] += 1;
}
