int cmp(int x, int y) { return x - y; }
int ge(int x, int y) { return x >= y; }
int eq(int x, int y) __attribute__((alias("__regcall3__cmp")));
int gt(int x, int y) __attribute__((alias("__regcall3__ge")));
int main(void) { return eq(1, 1) == 0 && gt(2, 1) ? 0 : 1; }
