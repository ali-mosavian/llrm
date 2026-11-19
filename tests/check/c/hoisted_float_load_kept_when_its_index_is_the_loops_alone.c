// RUN: llrm-cc %s -m16 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// The hoisted `x[i]` and `y[i]` address through an index the loop never reads:
// made again in the loop they keep it alive, and at m16's four address
// registers each trip pays a `mov bx` to reach them. They stay on the stack.
// CHECK-LABEL: _f@3 proc far
// CHECK: fsub st(0), st({{[0-9]}})
// CHECK-NOT: fsubr
// CHECK: _f@3 endp
double f(double *x, double *y, unsigned short i, unsigned short n)
{
    double s = 0.0;
    unsigned short j;
    for (j = i + 1; j < n; ++j) {
        double dx = x[j] - x[i], dy = y[j] - y[i];
        s += dx * dx + dy * dy;
    }
    return s;
}
