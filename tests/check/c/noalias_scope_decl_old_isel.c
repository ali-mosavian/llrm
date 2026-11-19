// RUN: llrm-cc %s -m16 -march=i386 -O2 --isel=old -S -o /dev/stdout
// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 --isel=old -S -o /dev/stdout
// Inlining a function with `restrict` parameters leaves `llvm.experimental.noalias.scope.decl` calls (#7043); the old selector rejected them
// ("assembles: @fn: @llvm.experimental.noalias.scope.decl", 11+ nib/qb fixtures). Needs #7408 (w12/old-isel-overflow) on release/next.
// CHECK: endp
static inline void add(int *restrict a, const int *restrict b, int n)
{
    for (int i = 0; i < n; i++)
        a[i] += b[i];
}
void use(int *x, int *y, int n)
{
    add(x, y, n);
    add(y, x, n);
}
