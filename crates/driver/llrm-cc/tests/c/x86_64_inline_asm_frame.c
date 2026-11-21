extern int g(int);

static inline int rint32(float x)
{
    int v;
    __asm__("cvtss2si %1, %0" : "=r"(v) : "x"(x));
    return v;
}

int f(float x, int n)
{
    int s = 0;
    for (int i = 0; i < n; i++)
        s += g(rint32(x + i));
    return s;
}
