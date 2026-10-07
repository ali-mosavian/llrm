struct S { int a, b, c; };
struct W { int a, b; };
struct S fs(int a, int b) { struct S s; s.a = a; s.b = b; s.c = a + b; return s; }
struct W fw(int a, int b) { struct W s; s.a = a; s.b = b; return s; }
extern struct S gs(int a, int b);
int callsr(int x) { struct S s = gs(x, 2); return s.c; }
