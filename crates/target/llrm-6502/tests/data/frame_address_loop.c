extern void use(char *);
int f(int n)
{
    char a[70], b[70];
    int i;
    use(a);
    for (i = 0; i < (n & 63); i++) b[i] = a[i] + 1;
    use(b);
    return b[3];
}
