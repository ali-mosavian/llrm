void sink(int *);
int leaf(int n) {
    volatile int a[8];
    for (int i = 0; i < 8; i++) a[i] = n + i;
    return a[n & 7];
}
int caller(int n) {
    int a[8];
    sink(a);
    return a[n & 7];
}
