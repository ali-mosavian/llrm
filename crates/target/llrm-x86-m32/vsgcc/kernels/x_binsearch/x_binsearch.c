extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 a[512];
long bench_x_binsearch(i32 n)
{
    i32 i, lo, hi, mid, key; long found = 0; u32 s = 5;
    for (i = 0; i < 512; ++i) a[i] = i * 3 + 1;
    for (i = 0; i < n; ++i) {
        s = s * 1103515245ul + 12345ul; key = (i32)((s >> 16) % 1600);
        lo = 0; hi = 511;
        while (lo <= hi) { mid = (lo + hi) >> 1; if (a[mid] < key) lo = mid + 1; else if (a[mid] > key) hi = mid - 1; else { found += mid; break; } }
    }
    return found;
}

int main(void)
{
    report(bench_x_binsearch(2000));
    return 0;
}
