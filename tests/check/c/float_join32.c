// RUN: llrm-c %s -m32 -mabi=sysv -O1 -march=i486 -S -o /dev/stdout
// A float a call returns on one path and a parameter is on the other, joined and returned: both stay in st(0).
// It was stored to a frame cell on both paths and loaded again at the join.
extern double g(double);
double pick(int n, double a)
{
    double r;
    if (n)
        r = g(a);
    else
        r = a;
    return r;
}
// CHECK-LABEL: _pick proc
// CHECK: call _g
// CHECK-NOT: fstp qword ptr
// CHECK-NOT: fld qword ptr
// CHECK: ret
// CHECK: _pick endp
