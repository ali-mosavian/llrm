struct S1 { char x; };
struct S12 { int a, b, c; };
struct S1 r1(int a) { struct S1 s; s.x = a; return s; }
struct S12 r12(int a, int b) { struct S12 s; s.a = a; s.b = b; s.c = a + b; return s; }
int by_val(struct S12 s, struct S1 t, int a) { return s.a + s.c + t.x + a; }
int sc(char a, short b, unsigned char c) { return a + b + c; }
int callsr(void) { struct S12 s = r12(1, 2); struct S1 t = r1(3); return s.c + t.x; }
extern struct S12 away(int a, int b);
int calls(int a) { struct S12 s = away(a, 2); return s.c; }
