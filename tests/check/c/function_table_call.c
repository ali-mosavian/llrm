// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// A call through a table of four functions is a switch of direct calls (jumptableswitch@672), and the pin's inliner, which repeats
// while the simplification makes calls direct (DevirtSCCRepeatedPass<4>), takes them: clang's loop holds the four bodies. x_funcptr
// had four call sites and code 1.60 over clang.
// CHECK-LABEL: _bench_x_funcptr proc
// CHECK-NOT: call
// CHECK: _bench_x_funcptr endp
extern void report(long value);

static int f0(int x) { return x + 3; }
static int f1(int x) { return x * 5; }
static int f2(int x) { return x ^ 0x55; }
static int f3(int x) { return x >> 1; }
static int (*table[4])(int) = { f0, f1, f2, f3 };

long bench_x_funcptr(int n)
{
    int i, x = 1;
    long sum = 0;
    for (i = 0; i < n; ++i) {
        x = table[(i * 7 + (x & 3)) & 3](x) & 0xFFFF;
        sum += x;
    }
    return sum;
}

int main(void)
{
    report(bench_x_funcptr(1500));
    return 0;
}
