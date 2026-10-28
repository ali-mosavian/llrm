// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=old -S -o /dev/stdout
// Float parameters read in a loop: an incoming stack argument is an immutable cell, so the pin remakes `fld [bp+N]` where it is read.
// `argument_loads` took only integer `mov`, so x and y were stored to fresh slots every iteration: a 30 B frame instead of 10 and 3 `fstp` in the loop.
// CHECK-LABEL: _f@3 proc
// CHECK: sub sp, 10
// CHECK: fld dword ptr [bp+6]
// CHECK: fld dword ptr [bp+10]
// CHECK: _f@3 endp
void use(int);
float f(int n, float x, float y)
{
    float s = 1;
    int i;
    for (i = 0; i < n; ++i) {
        use(i);
        s = s * x + y;
    }
    return s;
}
