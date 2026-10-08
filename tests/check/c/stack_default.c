// RUN: llrm-c %s -O2 -S -o /dev/stdout
// The default build checks nothing: no limit word, no overflow routine.
// CHECK-LABEL: deep proc
// CHECK-NOT: _llrm_os_stack_low
// CHECK-NOT: __STKOVERFLOW
// CHECK: deep endp
extern void report(long value);
int deep(int n)
{
    volatile char pad[48];
    pad[0] = (char)n;
    if (n == 0) return 0;
    return 1 + deep(n - 1) + pad[0] - (char)n;
}
int main(void) { report(deep(3)); return 0; }
