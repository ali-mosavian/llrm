// RUN: llrm-c %s -O2 -fsanitize=stack -S -o /dev/stdout
// -fsanitize=stack: a function compares SP with the OS layer's _llrm_os_stack_low once its frame is
// allocated and calls __STKOVERFLOW out of line, last.
// CHECK-LABEL: deep{{(@3)?}} proc
// CHECK: cmp sp, word ptr _llrm_os_stack_low
// CHECK-NEXT: jb
// CHECK: call far ptr __STKOVERFLOW
// CHECK-NEXT: deep{{(@3)?}} endp
extern void report(long value);
int deep(int n)
{
    volatile char pad[48];
    pad[0] = (char)n;
    if (n == 0) return 0;
    return 1 + deep(n - 1) + pad[0] - (char)n;
}
int main(void) { report(deep(3)); return 0; }
