extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 weight(i32 k)
{
    switch (k) {
    case 0: return 3;
    case 1: return 14;
    case 2: return 15;
    case 3: return 92;
    case 4: return 65;
    case 5: return 35;
    case 6: return 89;
    case 7: return 79;
    case 8: return 32;
    case 9: return 38;
    case 10: return 46;
    case 11: return 26;
    default: return 1;
    }
}
long bench_x_switch_dense(i32 n)
{
    i32 i; long sum = 0; u32 r = 5;
    for (i = 0; i < n; ++i) { r = r * 1103515245ul + 12345ul; sum += weight((i32)((r >> 16) % 14)); }
    return sum;
}

int main(void)
{
    report(bench_x_switch_dense(2000));
    return 0;
}
