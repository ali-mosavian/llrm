// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 --isel=dag -S -o /dev/stdout
// Two selects on one compare, both carried round the loop, were two diamonds: the copy that exports the first result to its virtual register sat between
// the two selects (it was made as `mov`, not defined in place), so EmitLoweredSelect's run was broken (x_heapsort sift: +1 branch per trip, dag/pin 1.057).
int f(int *a, int n)
{
    int m = 0, j = 0, i;
    for (i = 0; i < n; i++) {
        int v = a[i];
        int c = v > m;
        m = c ? v : m;
        j = c ? i : j;
    }
    return m * 100 + j;
}
// CHECK-LABEL: _f proc
// CHECK: inc
// CHECK: jg
// CHECK-NOT: jg
// CHECK: _f endp
