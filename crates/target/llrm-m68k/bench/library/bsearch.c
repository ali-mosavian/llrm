int k(const int *a, int n, int key) {
    int lo = 0, hi = n - 1;
    while (lo <= hi) {
        int m = (lo + hi) >> 1;
        if (a[m] == key) return m;
        if (a[m] < key) lo = m + 1; else hi = m - 1;
    }
    return -1;
}
