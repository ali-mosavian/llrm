struct s10 { int x; long y; long z; };
struct s10 a2[562];
int a3[562];
struct s10 a4[562];
long fivestreams(int __far *a1, int *a5, int n, int m)
{
    long s;
    int k;
    struct s10 *p0;
    int __far *p1;
    struct s10 *p2;
    int *p3;
    struct s10 *p4;
    int *p5;
    struct s10 *e;
    for (k = 0; k < 3; k++) {
        p1 = &a1[n + k * m + 21];
        p2 = &a2[k * m + 24];
        p3 = &a3[n + k * m + 24];
        p4 = &a4[1 + k * m + 21];
        p5 = &a5[k * m + 26];
        while (p0 < e) {
            p0[0].x = *p1;
            s += p2[0].x;
            s += *p3;
            s += p4[0].x;
            s += *p5;
            p0 += 2;
            p1 += 1;
            p2 += 2;
            p3 += 1;
            p4 += 1;
            p5 += 1;
        }
    }
    return s;
}
