int g;
int f(int *p, int n) {
    int a[64], s = 0, i;
    for (i = 0; i < n; i++) { p[i] = g; a[i] = i; s += g; }
    return s + a[n-1];
}
