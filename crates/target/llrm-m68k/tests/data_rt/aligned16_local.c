extern void keep(long *p);
void f(void) { long m[8] __attribute__((aligned(16))); for (int j = 0; j < 8; ++j) m[j] = 0; keep(m); }
