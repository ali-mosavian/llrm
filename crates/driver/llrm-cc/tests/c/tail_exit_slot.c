extern int g(int);
extern int g4(int, int, int, int);
int tail(int x) { return g(g(x)); }
int tail4(int a, int b, int c, int d) { return g4(a, b, c, d) + 0 * g4(d, c, b, a); }
int mixed(int a, int x) { if (x) return g4(a, a, a, a); return g4(a, 1, 2, 3) + 1; }
