int g(int);
int f(int a) { int r = 0; { int t = g(a) + 1; r = g(t); } for (int i = 0; i < a; i++) { int u = g(i); r += u; } return r; }
