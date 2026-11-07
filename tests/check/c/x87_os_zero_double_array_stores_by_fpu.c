// RUN: llrm-cc %s -m32 -Os -march=i486 --isel=dag -S -o /dev/stdout
// Without SSE a zeroed double array at -Os is `fldz` and one `fst qword` per
// double (4 bytes at a short displacement), not two `mov dword` zero stores
// (8): nbody m32 -Os spent 127 bytes on such fills, gcc 92.
extern void use(double *);
void kernel(void)
{
    double a[4] = {0, 0, 0, 0};
    use(a);
}
// CHECK-LABEL: kernel_ proc
// CHECK: fldz
// CHECK-NOT: mov dword ptr [esp
