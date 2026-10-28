#include <stdint.h>
#include <stdio.h>

int32_t eq_or(int32_t);
int32_t and_and(int32_t, int32_t);
int32_t mixed(int32_t, int32_t, int32_t);
int32_t negative_and_wide_constants(int32_t, int32_t);
int32_t three_ors(int32_t, int32_t, int32_t);
int64_t wide(int64_t, int64_t);
int32_t narrow(int8_t, int8_t);
int64_t shared_leaf(int32_t, int64_t *);

static const int32_t grid[] = {INT32_MIN, INT32_MIN + 1, -100, -33, -32, -31, -6, -5, -4, -1, 0, 1, 3, 4, 8, 9, 10,
                               30, 31, 32, 33, 72, 79, 100, 101, 200, INT32_MAX - 1, INT32_MAX};
#define N ((int)(sizeof grid / sizeof grid[0]))
static int bad;
#define CHECK(name, got, want, a, b, c)                                                              \
    if ((got) != (want)) {                                                                           \
        printf("%s(%lld, %lld, %lld): %lld, want %lld\n", name, (long long)(a), (long long)(b),      \
               (long long)(c), (long long)(got), (long long)(want));                                 \
        bad = 1;                                                                                     \
    }

int main(void) {
    for (int i = 0; i < N; i++) {
        int32_t x = grid[i];
        CHECK("eq_or", eq_or(x), (x == 72 || x == 79) ? 2 : 1, x, 0, 0);
        int64_t stored = 0;
        CHECK("shared_leaf", shared_leaf(x, &stored), x == 72 ? 16 : 8, x, 0, 0);
        CHECK("shared_leaf stored", stored, (x == 72 || x == 79) ? 2 : 1, x, 0, 0);
        for (int j = 0; j < N; j++) {
            int32_t y = grid[j];
            CHECK("and_and", and_and(x, y), (x > 3 && y < 9) ? x : y, x, y, 0);
            int cond = x > -5 || (uint32_t)y < 31 || (x != 32 && y <= -31);
            CHECK("negative_and_wide_constants", negative_and_wide_constants(x, y), cond ? x : y, x, y, 0);
            CHECK("wide", wide(x, y), (x >= y && x != 0) ? (int64_t)x : (int64_t)y, x, y, 0);
            CHECK("wide64", wide((int64_t)x * 4096, y), ((int64_t)x * 4096 >= y && x != 0) ? (int64_t)x * 4096 : y, x, y, 0);
            CHECK("narrow", narrow((int8_t)x, (int8_t)y), ((int8_t)x > -3 && (uint8_t)(int8_t)y < 200), x, y, 0);
            for (int k = 0; k < N; k++) {
                int32_t z = grid[k];
                CHECK("mixed", mixed(x, y, z), ((x == 5 && y != 7) || (uint32_t)z > 100) ? x : z, x, y, z);
                CHECK("three_ors", three_ors(x, y, z), (x < y || (uint32_t)y >= (uint32_t)z || z == -32) ? x : z, x, y, z);
            }
        }
    }
    puts(bad ? "FAIL" : "ok");
    return bad;
}
