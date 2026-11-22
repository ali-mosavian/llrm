extern void use(char *);
int f(int n, int *p)
{
    char a[70], b[70];
    int i, s0 = 0, s1 = 1, s2 = 2, s3 = 3, s4 = 4, s5 = 5, s6 = 6, s7 = 7, s8 = 8;
    use(a);
    use(b);
    for (i = 0; i < (n & 63); i++) {
        s0 += a[i]; s1 ^= s0; s2 += s1; s3 ^= s2; s4 += s3;
        s5 ^= s4; s6 += s5; s7 ^= s6; s8 += s7 + p[i];
    }
    return s0 + s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;
}
