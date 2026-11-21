extern int f3(int, int, int); extern int f2(int, int); extern int f1(int);
int g(int a, int b, int c) { int x = f3(a, b, c); x += f3(b, c, a); x += f2(a, b); x += f2(b, c); x += f1(a); return x + f3(c, a, 7); }
