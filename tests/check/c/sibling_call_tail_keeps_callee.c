// RUN: llrm-cc %s -O2 --isel=dag -S -o /dev/stdout
// Placement copied the shared tail `jmp _g` into a parent under the jump's id, where the callee table has no entry: the copy printed a bare
// `jmp` (no target) in f2. DEVIATE TD1: a sibling call's tail is not copied.
// CHECK-LABEL: {{_f2(@3)?}} proc
// CHECK: jmp far ptr _g@3
// CHECK-NOT: {{jmp$}}
// CHECK: {{_f2(@3)?}} endp
extern int g(int);
int f2(int a, int b)
{
    if (a > b)
        return g(a);
    if (a < b)
        return g(b);
    return g(0);
}
