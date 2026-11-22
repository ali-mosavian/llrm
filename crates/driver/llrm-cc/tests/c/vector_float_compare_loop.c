/* The compare loop of sfm16_float_cmp: six boolean terms of two strided floats, NaN lanes included. At -O2 on x86-64
   the loop vectorizer widens it (`cmpps`, a mask `psubd`) where it had no priced vector compare, lane insert or mask
   extension and stayed scalar (3x clang's instructions); the lane extracts of the tail must also select. */
#include <stdio.h>
#include <math.h>
struct row { float v[11]; };
static struct row V[67];
__attribute__((noinline)) long run(int n)
{
    long acc = 0;
    for (int i = 0; i < n; ++i) {
        float a = V[i].v[0], b = V[i].v[1];
        acc += (a < b) + 2 * (a == b) + 4 * (a > b) + 8 * (a != b) + 16 * (a <= b) + 32 * (a >= b);
    }
    return acc;
}
int main(void)
{
    for (int i = 0; i < 67; ++i) {
        V[i].v[0] = (i % 5 == 0) ? NAN : (float)(i % 7);
        V[i].v[1] = (i % 11 == 0) ? NAN : (float)(i % 4);
    }
    printf("%ld %ld %ld\n", run(67), run(8), run(1));
    return 0;
}
