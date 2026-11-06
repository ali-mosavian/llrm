int c0(int, int);
int c1(int, int);
int g(int a, int b)
{
    if (b != 5 && (b & 4)) return c0(a, b);
    else if (b >= 3 || (a & 1)) return c1(a, b);
    else if (a == 4 && (b & 4)) return c0(a, b);
    else if (a < 8) return c1(a, b);
    return 0;
}
