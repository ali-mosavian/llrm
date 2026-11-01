int k(int *p, int n, int t) { int s = 0, i = 0; do { s += p[i] > t; i++; } while (i < n); return s; }
