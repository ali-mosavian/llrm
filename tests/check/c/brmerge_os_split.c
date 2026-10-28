// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// Two compares joined by `||` at -Os were merged into one condition: `setbe`, `setae`, a logic op and one branch, in byte registers the 16-bit
// target has few of (sieve: +17% clocks, QCport -Os +0.33%). They stay two compare-and-branch pairs (DEVIATE SD103).
// CHECK-LABEL: _f@3 proc
// CHECK-NOT: set
// CHECK: cmp al, 5
// CHECK-NEXT: ja
// CHECK: cmp dl, 3
// CHECK-NEXT: jb
// CHECK: _f@3 endp
extern void g(void);
void f(unsigned char a, unsigned char b)
{
    if (a > 5 || b < 3)
        g();
}
