// RUN: llrm-cc %s -m32 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// Only -Os fills a zero array through the FPU: `fst qword` is slower than two
// `mov dword` on a 486 and the byte rate does not repay it at -O2.
extern void use(double *);
void kernel(void)
{
    double a[4] = {0, 0, 0, 0};
    use(a);
}
// CHECK-LABEL: kernel_ proc
// CHECK: mov dword ptr [esp
// CHECK-NOT: fldz
