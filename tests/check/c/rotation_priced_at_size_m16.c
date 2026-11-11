// RUN: llrm-cc %s -m16 -march=i486 -Os -fno-inline-functions -S -o /dev/stdout
// recpow -Os: `power` as a loop was rotated, copying its three-way header (`test dl, 1`, the multiply setup, `sar`) in front of the loop,
// 192 B; kept unrotated it is 173 B. A size build rotates only where the copied header is no bigger than the branches rotation removes
// (arm64's w15 rule, `rotation_header_priced_at_size`); the same rule shrank six BASIC programs -4..-60 B at m16.
// CHECK-LABEL: _power@3 proc
// CHECK: test cl, 1
// CHECK-NOT: test dl, 1
// CHECK: _power@3 endp
static long power(long b, int e)
{
    if (e == 0) return 1;
    if (e & 1) return b * power(b, e - 1) % 30011;
    return power(b * b % 30011, e >> 1);
}

long bench_recpow(int n)
{
    long total = 0;
    int i;

    for (i = 1; i <= n; ++i) total += power(i % 97 + 2, i + 40) % 1000;
    return total;
}
