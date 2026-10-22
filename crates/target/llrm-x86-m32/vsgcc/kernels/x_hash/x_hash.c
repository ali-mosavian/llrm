extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 keys[256], vals[256];
static u32 hash(i32 k) { u32 h = (u32)k * 2654435761ul; return h >> 24; }
long bench_x_hash(i32 n)
{
    i32 i, slot; long sum = 0;
    for (i = 0; i < 256; ++i) keys[i] = -1;
    for (i = 0; i < n; ++i) {
        i32 k = (i * 97) % 211; slot = (i32)hash(k);
        while (keys[slot] != -1 && keys[slot] != k) slot = (slot + 1) & 255;
        if (keys[slot] == -1) { keys[slot] = k; vals[slot] = i; } else vals[slot] += i;
    }
    for (i = 0; i < 256; ++i) if (keys[i] != -1) sum += vals[i] ^ keys[i];
    return sum;
}

int main(void)
{
    report(bench_x_hash(600));
    return 0;
}
