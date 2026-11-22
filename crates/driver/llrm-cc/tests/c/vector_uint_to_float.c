/* `<4 x i32>` to `<4 x float>`, unsigned: every lane from 0 to 2^32 - 1 rounds as the scalar convert does. */
#include <stdio.h>

typedef unsigned u4 __attribute__((vector_size(16)));
typedef float f4 __attribute__((vector_size(16)));

__attribute__((noinline)) void convert(const u4 *p, f4 *q) { *q = __builtin_convertvector(*p, f4); }

int main(void)
{
    static const unsigned v[][4] = {
        {0, 1, 65535, 65536}, {0x7fffffffu, 0x80000000u, 0x80000001u, 0xffffffffu}, {16777217u, 3000000000u, 0x12345678u, 0xfffffffeu},
    };
    int i, k;

    for (i = 0; i < 3; i++) {
        u4 in = {v[i][0], v[i][1], v[i][2], v[i][3]};
        f4 out;
        float lane[4];

        convert(&in, &out);
        __builtin_memcpy(lane, &out, sizeof lane);
        for (k = 0; k < 4; k++)
            printf("%u %.1f\n", v[i][k], (double)lane[k]);
    }
    return 0;
}
