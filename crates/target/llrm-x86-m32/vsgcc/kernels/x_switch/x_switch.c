extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 step(i32 s, i32 c)
{
    switch (s) {
    case 0: return c < 64 ? 1 : (c < 128 ? 2 : 0);
    case 1: return c & 1 ? 3 : 0;
    case 2: return c > 200 ? 4 : 1;
    case 3: return (c & 6) == 6 ? 2 : 5;
    case 4: return c ^ 3 ? 0 : 5;
    default: return c < 10 ? 0 : 3;
    }
}
long bench_x_switch(i32 n)
{
    i32 i, s = 0; long sum = 0; u32 r = 9;
    for (i = 0; i < n; ++i) { r = r * 1103515245ul + 12345ul; s = step(s, (i32)((r >> 16) & 255)); sum += s * (i & 3); }
    return sum;
}

int main(void)
{
    report(bench_x_switch(1500));
    return 0;
}
