void f(void); void h(void);
void g(int a, int b){ if (a) { if (b) f(); else h(); } else { if (b) h(); else f(); } }
