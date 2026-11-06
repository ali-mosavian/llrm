int g(int *);
int leaf(int x) { return x * 3 + 1; }
int frame(int x) { int a[8]; for (int i = 0; i < 8; i++) a[i] = x + i; return g(a); }
int calls(int x) { return g(&x) + g(&x); }
int many(int a, int b, int c, int d, int e) { return g(&a) + g(&b) + g(&c) + g(&d) + g(&e); }
double fp(double x, double y) { extern double h(double); return h(x) + h(y) * x; }
int framereg(long n) { int *p = __builtin_alloca(n * 4); p[0] = (int)n; return g(p); }
double xmms(double a, double b, double c) { extern double h(double); double x = h(a), y = h(b), z = h(c); return x * y + z * a + b; }
int probed(int x) { char big[8192]; big[x & 7] = 1; return g((int *)big); }
