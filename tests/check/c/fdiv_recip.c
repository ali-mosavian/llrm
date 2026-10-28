// RUN: llrm-cc %s -m32 -Os --isel=dag -S -o /dev/stdout
// `x / 2.0` was an x87 `fdiv`; the pin's visitFDIV makes it `x * 0.5`.
double half(double x)
{
    return x / 2.0;
}
// CHECK-LABEL: half_ proc
// CHECK-NOT: fdiv
// CHECK: fmul
