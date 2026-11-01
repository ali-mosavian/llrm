extern int *p; void f(void); void g(void){ if (*p == 0) f(); }
