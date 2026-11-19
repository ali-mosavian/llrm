void use(int *);
void g(int n) { int *p = __builtin_alloca(n); use(p); }
