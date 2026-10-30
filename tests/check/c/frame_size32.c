// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 -S -o /dev/stdout
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
// The pin (none-elf llc, optsize) keeps no frame register here: `sub esp` and `add esp` cost less than `push ebp; mov ebp, esp; leave`.
// CHECK-NOT: push ebp
// CHECK-NOT: leave
// CHECK: _big endp
