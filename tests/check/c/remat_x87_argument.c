// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=old -S -o /dev/stdout
// Float parameters read in a loop: an incoming stack argument is an immutable cell, so the pin remakes `fld [bp+N]` where it is read.
// `argument_loads` took only integer `mov`, so x and y were stored to fresh slots every iteration: a 30 B frame instead of 10 and 3 `fstp` in the loop.
// CHECK-LABEL: _f@3 proc
// Nothing is stored before the loop reads the arguments, and the loop stores one value: the product-sum, which crosses the call.
// CHECK-NOT: fstp
// CHECK: {{f(ld|mul)}} dword ptr [bp+6]
// CHECK: {{f(ld|add)}} dword ptr [bp+10]
// CHECK: fstp dword ptr [bp-{{[0-9]+}}]
// CHECK-NOT: fstp
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
