struct S { int a, b, c; };
struct T { char x; };
int six(int a, int b, int c, int d, int e, int f) { return a + b + c + d + e + f; }
long long wide(int a, long long b, int c) { return a + b * 3 + c; }
struct S result(int a, int b) { struct S s; s.a = a; s.b = b; s.c = a + b; return s; }
int by_value(struct S s, int a) { return s.a + s.c + a; }
int small(struct T t, int a) { return t.x + a; }
int __cdecl explicit_cdecl(int a, int b) { return a - b; }
extern int far_away(int a, int b, int c);
int call(int a) { return far_away(a, 2, 3) + six(1, 2, 3, 4, 5, a); }
