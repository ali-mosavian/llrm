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
u8 a0[300];
i16 a1[300];
i32 g2[300];
double FAR a3[300];
struct s6 a4[300];
struct s10 a5[300];
u8 g6[300];
i16 FAR a7[300];
i32 a8[300];
i32 f(u8 *a0, double FAR *a3, struct s6 *a4, i16 FAR *a7, i32 *a8, i16 n, i16 o, i16 m)
{
    i32 a2[300];
    u8 a6[300];
    i16 i;
    i32 s;
    i32 u;
    i32 j;
    i32 q;
    i32 d;
    i16 k;
    lcopy((void FAR *)a2, (void FAR *)g2, 1200u);
    lcopy((void FAR *)a6, (void FAR *)g6, 300u);
    s = 0L;
    u = 1L;
    j = 0L;
    q = 0L;
    i = 0;
    for (i = 0; i < n; i++) {
        s += ((i32)a0[(i + 8)]);
        s += ((i32)a1[(i + 8)]);
        s += ((i32)a2[(i + 8)]);
        s += ((i32)a3[(i + 8)]);
        s += ((i32)a4[(i + 8)].x);
        s += ((i32)a5[(i + 8)].x);
        s += ((i32)a6[(i + 8)]);
        s += ((i32)a7[(i + 8)]);
        s += ((i32)a8[(i + 8)]);
    }
    return ((s + u) + (j + q));
}
