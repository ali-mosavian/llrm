static __inline int twice(int x) { return x + x; }
int three(int r) { return twice(r) + twice(r + 1); }
