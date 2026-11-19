// RUN: llrm-cc %s -m32 -Os -march=i486 --isel=dag -S -o /dev/stdout
// Without SSE a zeroed double array at -Os is never a run of `mov dword` zero
// stores (nbody m32 -Os spent 127 bytes on those, gcc 92). It is `fldz` and
// `fst qword` per double, or `rep stosd` where the size cost model finds that
// shorter (#7772: 32 bytes against 33 here).
extern void use(double *);
void kernel(void)
{
    double a[4] = {0, 0, 0, 0};
    use(a);
}
// CHECK-LABEL: kernel_ proc
// CHECK-NOT: mov dword ptr [esp
// CHECK: {{fldz|rep stosd}}
