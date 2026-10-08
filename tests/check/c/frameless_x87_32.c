// RUN: llrm-c %s -m32 -mabi=sysv -O1 -march=i486 -S -o /dev/stdout
// x87 control-word and register-exchange instructions leave the stack pointer alone: a function using them keeps no frame register.
// `fnstcw` and `fxch` were refused, so every float-to-integer conversion kept `push ebp; mov ebp, esp` (bench/recfloat).
extern double g(double, double);
long truncated(double x)
{
    return (long)(x * 1000.0 + 0.5);
}
double swapped(double a, double b)
{
    return g(b * 0.5 + 1.0, a + b);
}
// CHECK-LABEL: _truncated proc
// CHECK-NOT: ebp
// CHECK: _truncated endp
// CHECK-LABEL: _swapped proc
// CHECK-NOT: ebp
// CHECK: _swapped endp
