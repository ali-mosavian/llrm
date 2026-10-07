int far callee(int x) { return x + 1; }
int near inner(int x) { return x + 2; }
int (far *fp)(int) = callee;
int (near *np)(int) = inner;
int apply(int (far *f)(int), int v) { return f(v); }
int main(void) { return apply(fp, 5) + (*np)(1); }
