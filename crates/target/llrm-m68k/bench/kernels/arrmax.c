int k(int *p, int n) { int m = p[0], i = 1; do { if (p[i] > m) m = p[i]; i++; } while (i < n); return m; }
