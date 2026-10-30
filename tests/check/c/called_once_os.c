// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// A static called twice, ~12 operations, at -Os: the pin prices the first site 45 against its threshold of 50 and inlines it, then the second as
// the last call (clang -Os -Rpass=inline). D11 (a callee that stays must not outgrow its call) is the 16-bit target's only (`inline_size_guard`), so m32 follows the pin.
// CHECK-LABEL: _g proc
// CHECK-NOT: call
// CHECK: _g endp
static int sum(const int *a, int n)
{
    int s = 0, i;
    for (i = 0; i < n; ++i) {
        if (a[i] > 3) s += a[i] * a[i]; else s -= a[i];
        s ^= s >> 3;
        s += (a[i] & 7) * (s | 1);
    }
    return s;
}
static int twice(const int *a, int n)
{
    int s = 0, i;
    for (i = 0; i < n; ++i) {
        if (a[i] > 5) s += a[i] * 3; else s -= a[i];
        s ^= s >> 2;
        s += (a[i] & 3) * (s | 1);
    }
    return s;
}
int f(const int *a, int n) { return sum(a, n) + 1; }
int g(const int *a, int n) { return twice(a, n) + twice(a + 1, n - 1); }
