int g(int); int h(int);
int f(int n) { if (n == 0) return 0; int a = g(n), b = h(n); return a * b + g(a) + h(b) + a; }
int f2(int n, int m) { if (n < 0) return -1; if (m == 0) return 0; int a = g(n), b = h(m); return a + b + g(a + b); }
int f3(int *p, int n) { if (!p) return 0; int s = 0; for (int i = 0; i < n; i++) s += g(p[i]) * h(i); return s; }
