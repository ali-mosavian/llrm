typedef unsigned u32; typedef int s32;
static volatile u32 v[14];
static inline __attribute__((always_inline)) u32 branches(u32 a, u32 b) {
    s32 x = (s32)a, y = (s32)b;
    u32 r = 0;
    if (a < b) r += 1;
    if (a <= b) r += 2;
    if (a > b) r += 4;
    if (a >= b) r += 8;
    if (x < y) r += 16;
    if (x <= y) r += 32;
    if (x > y) r += 64;
    if (x >= y) r += 128;
    return r;
}
u32 f(u32 i) {
    u32 h = 0;
    h = h * 257u + branches(v[i], v[0]);
    h = h * 257u + branches(v[i], v[1]);
    h = h * 257u + branches(v[i], v[2]);
    h = h * 257u + branches(v[i], v[3]);
    h = h * 257u + branches(v[i], v[4]);
    h = h * 257u + branches(v[i], v[5]);
    h = h * 257u + branches(v[i], v[6]);
    h = h * 257u + branches(v[i], v[7]);
    h = h * 257u + branches(v[i], v[8]);
    h = h * 257u + branches(v[i], v[9]);
    h = h * 257u + branches(v[i], v[10]);
    h = h * 257u + branches(v[i], v[11]);
    h = h * 257u + branches(v[i], v[12]);
    h = h * 257u + branches(v[i], v[13]);
    return h;
}
