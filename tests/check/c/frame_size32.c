// RUN: llrm-c %s -m32 -mabi=sysv -Os -march=i486 -S -o /dev/stdout
// Tuned for size the frame register stays where the cells it names cost less than the stack pointer: [esp+d] is a byte longer than [ebp+d].
// Seven cells addressed through esp took 21 bytes more than the `push ebp; mov ebp, esp; leave` they saved (bench/nbody -Os +12%).
extern void use(double *);
double big(double x)
{
    double a[12];
    a[0] = x;
    a[11] = x * 2.0;
    use(a);
    return a[0] + a[11] + a[5] + a[7] + a[3];
}
// CHECK-LABEL: _big proc
// CHECK: push ebp
// CHECK: leave
// CHECK: _big endp
