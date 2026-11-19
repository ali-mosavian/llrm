int k(int (*f)(int), int x) { return f(f(x)) + f(x + 1); }
