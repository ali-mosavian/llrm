// RUN: llrm-cc %s -m32 -mabi=sysv -O3 -march=i486 --isel=dag -S -o /dev/stdout
// CHECK: _main proc near
// CHECK: _main endp
// CHECK-NOT: proc near
// CHECK: end
// -O3 gave the call chop(0, 64) a constant-argument clone (ipa-cp-clone, gcc's -O3 flag; the pin's default<O3> has none) and the unroller expanded
// the clones: 1380 lines of assembly where the pin's own -O3 IR is 171. A clone is a second proc after main.
extern void report(long value);

static int values[64];

static long chop(int lo, int hi)
{
    long sum = 0;
    int i, mid;

    if (hi - lo < 2) return 0;
    mid = (lo + hi) / 2;
    for (i = lo; i < hi; ++i) sum += values[i] & mid;
    return sum + chop(lo, mid) + chop(mid, hi);
}

long bench_recchop(int n)
{
    int i;

    for (i = 0; i < 64; ++i) values[i] = (i * 37 + n) & 127;
    return chop(0, 64);
}

int main(void)
{
    report(bench_recchop(5));
    return 0;
}
