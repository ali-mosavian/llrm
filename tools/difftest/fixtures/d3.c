int f(int); int h(int);
int g(int a, int b){ if (a) return f(b+1); if (b) return h(b+1); return f(b+1); }
