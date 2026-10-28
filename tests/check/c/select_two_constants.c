// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 --isel=dag -S -o /dev/stdout
// select(c, -1, 1) stayed a branch (mov -1; jl; mov 1). The pin folds it to sext(setcc) | 1 (foldSelectOfConstants).
int f(int a, int b) { return a < b ? -1 : 1; }
// CHECK-LABEL: _f proc
// CHECK: setl
// CHECK: neg
// CHECK: or {{.*}}, 1
// CHECK-NOT: jl
