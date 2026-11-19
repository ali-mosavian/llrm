int k(int *a, int n) {
    int pivot = a[n - 1], i = 0;
    for (int j = 0; j < n - 1; j++)
        if (a[j] < pivot) { int t = a[i]; a[i] = a[j]; a[j] = t; i++; }
    int t = a[i]; a[i] = a[n - 1]; a[n - 1] = t;
    return i + a[0] * 16;
}
