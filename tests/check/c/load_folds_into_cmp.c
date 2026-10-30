// RUN: llrm-cc %s -m32 -O1 -march=i486 --isel=dag -S -o /dev/stdout
// Both compares of `a[mid]` read one load, so the load stayed a `mov` and the second `cmp` stayed
// too: the pin's MachineCSE shares the flags across the sole-predecessor edge and its PeepholeOptimizer
// folds the single-use load into the one `cmp` (coalesce_hot_copies: 11 instructions a trip, pin 9).
int a[512];
long f(int key)
{
    int lo = 0, hi = 511, mid;
    while (lo <= hi) {
        mid = (lo + hi) >> 1;
        if (a[mid] < key) lo = mid + 1;
        else if (a[mid] > key) hi = mid - 1;
        else return mid;
    }
    return -1;
}
// CHECK-LABEL: f_ proc
// CHECK: cmp dword ptr _a[{{e[a-d]x}}*4], e{{[a-d]x}}
// CHECK-NEXT: jl
// CHECK-NEXT: {{L0_[0-9]+}}:
// CHECK-NEXT: jle
// CHECK-NOT: mov {{.*}}_a[
