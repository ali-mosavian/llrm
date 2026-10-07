struct S { int a, b, c; };
int six(int a, int b, int c, int d, int e, int f) { return a - b + c * d - e + f; }
long mixed(int a, long b, int c) { return a + b + c; }
int far_pointer(char __far *p, int a, int b) { return *p + a + b; }
struct S result(int a, int b) { struct S s; s.a = a; s.b = b; s.c = a + b; return s; }
extern long away(long a, int b);
long calls(long x) { return away(x, 5) + away(1, 2); }
