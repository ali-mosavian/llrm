int sum(int *p, int n) {
    int s = 0, i;
    for (i = 0; i < n; i++) s += p[i];
    return s;
}
