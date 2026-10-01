typedef signed char i8; typedef unsigned char u8; typedef int i16; typedef unsigned u16;
typedef long i32; typedef unsigned long u32;
#define FAR __far
#define HUGE __huge

extern void report(i32 v);
extern i16 keep(i16 x); extern i32 keep32(i32 x); extern void touch(void); extern void tick(void);
extern u16 tick_count(void); extern void lcopy(void FAR *d, void FAR *s, u16 n);
#pragma pack(1)
struct s6 { i16 x; i16 y; i16 z; };
struct s10 { i16 x; i32 y; i32 z; };
i16 f_rnd98_0304_a[4][760];
i32 f_rnd98_0304(i16 (*a)[760], i16 n, i16 b, i16 st, i16 m, i16 q)
{
    i16 i;
    i32 s;
    i32 j;
    i32 t;
    i16 r;
    i16 x;
    i16 e;
    s = 0L;
    j = 0L;
    t = 0L;
    i = 0;
    x = 0;
    for (x = 0; x < 4; x++) {
        for (i = 3; i <= 40; i += 3) {
            s += (((i32)a[x][i]) + ((i32)i));
        }
    }
    return (((s + j) + t) + ((i32)i));
}
u8 FAR a0[790];
struct s6 a1[790];
i32 FAR a2[790];
struct s6 a3[790];
i16 a4[790];
struct s10 FAR a5[790];
i16 a6[790];
i32 f(u8 FAR *a0, struct s6 *a1, i32 FAR *a2, struct s6 *a3, struct s10 FAR *a5, i16 n, i16 o, i16 m)
{
    i16 i;
    i32 s;
    i32 u;
    i32 j;
    i32 q;
    i32 d;
    i16 k;
    u8 FAR *p0;
    struct s6 *p1;
    i32 FAR *p2;
    struct s6 *p3;
    i16 *p4;
    struct s10 FAR *p5;
    i16 *p6;
    s = 0L;
    u = 1L;
    j = 0L;
    q = 0L;
    i = 0;
    p0 = &a0[((((n + 3) - 1) + (-1)) + 11)];
    p1 = &a1[((((n + 3) - 1) + o) + 8)];
    p2 = &a2[((2 * ((n + 3) - 1)) + 13)];
    p3 = &a3[((((n + 3) - 1) * m) + 8)];
    p4 = &a4[((2 * ((n + 3) - 1)) + 8)];
    p5 = &a5[((((n + 3) - 1) + n) + 8)];
    p6 = &a6[((((n + 3) - 1) + n) + 11)];
    for (i = ((n + 3) - 1); i >= 0; i--) {
        (*p0)++;
        (*p2)++;
        (*p4)++;
        (*p6)++;
        s += ((i32)p1[0].x);
        s += ((i32)p3[0].x);
        s += ((i32)p5[0].x);
        p0 = (p0 + (-1));
        p1 = (p1 + (-1));
        p2 = (p2 + (-2));
        p3 = (p3 + (m * (-1)));
        p4 = (p4 + (-2));
        p5 = (p5 + (-1));
        p6 = (p6 + (-1));
    }
    return ((s + u) + (j + q));
}
