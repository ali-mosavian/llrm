extern int e(int);
extern int f0(int, int);
extern int f1(int, int);
extern int f2(int, int);
extern int f3(int, int);
extern int f4(int, int);
extern int f5(int, int);
extern int f6(int, int);
extern int f7(int, int);
extern int f8(int, int);
extern int f9(int, int);
extern int f10(int, int);
extern int f11(int, int);
extern int f12(int, int);
extern int f13(int, int);
int dispatch(unsigned char *p, int x)
{
    int r = 0;
    switch (p[0]) {
    case 10:
        if (!p[1]) return e(0);
        r += f0(x, p[1]);
        if (r > 100) return e(r);
        break;
    case 17:
        if (!p[1]) return e(1);
        r += f1(x, p[1]);
        if (r > 101) return e(r);
        break;
    case 24:
        if (!p[1]) return e(2);
        r += f2(x, p[1]);
        if (r > 102) return e(r);
        break;
    case 31:
        if (!p[1]) return e(3);
        r += f3(x, p[1]);
        if (r > 103) return e(r);
        break;
    case 38:
        if (!p[1]) return e(4);
        r += f4(x, p[1]);
        if (r > 104) return e(r);
        break;
    case 45:
        if (!p[1]) return e(5);
        r += f5(x, p[1]);
        if (r > 105) return e(r);
        break;
    case 52:
        if (!p[1]) return e(6);
        r += f6(x, p[1]);
        if (r > 106) return e(r);
        break;
    case 59:
        if (!p[1]) return e(7);
        r += f7(x, p[1]);
        if (r > 107) return e(r);
        break;
    case 66:
        if (!p[1]) return e(8);
        r += f8(x, p[1]);
        if (r > 108) return e(r);
        break;
    case 73:
        if (!p[1]) return e(9);
        r += f9(x, p[1]);
        if (r > 109) return e(r);
        break;
    case 80:
        if (!p[1]) return e(10);
        r += f10(x, p[1]);
        if (r > 110) return e(r);
        break;
    case 87:
        if (!p[1]) return e(11);
        r += f11(x, p[1]);
        if (r > 111) return e(r);
        break;
    case 94:
        if (!p[1]) return e(12);
        r += f12(x, p[1]);
        if (r > 112) return e(r);
        break;
    case 101:
        if (!p[1]) return e(13);
        r += f13(x, p[1]);
        if (r > 113) return e(r);
        break;
    }
    return r;
}
