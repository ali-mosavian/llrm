int g(int *);
int leaf(int x) { return x * 3 + 1; }
int frame(int x) { int a[8]; for (int i = 0; i < 8; i++) a[i] = x + i; return g(a); }
int calls(int x) { return g(&x) + g(&x); }
int many(int a, int b, int c, int d, int e) { return g(&a) + g(&b) + g(&c) + g(&d) + g(&e); }
double fp(double x, double y) { extern double h(double); return h(x) + h(y) * x; }
