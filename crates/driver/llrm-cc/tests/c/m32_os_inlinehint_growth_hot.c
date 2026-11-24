/* A hinted callee of five operations called six times from a function `main` runs a thousand times:
 * keeping the calls costs clocks (-147 B for -1504 of 3465 work), so the copies stay. */
typedef unsigned int u32;
static inline int mix(u32 x)
{
    x ^= x >> 15;
    x *= 0x2c1b3c6du;
    x ^= x >> 12;
    x *= 0x297a2d39u;
    x ^= x >> 15;
    return (int)(x & 0x7fffffff);
}

__attribute__((noinline)) int step(u32 i)
{
    return mix(i) + mix(i + 1) + mix(i * 3) + mix(i ^ 5) + mix(i | 9) + mix(i - 7);
}
int main(void)
{
    int sum = 0;
    for (u32 i = 1; i < 1000; i++)
        sum += step(i);
    return sum;
}
