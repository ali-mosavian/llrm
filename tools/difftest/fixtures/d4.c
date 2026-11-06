void f(int); void h(int);
void g(int a){ switch (a) { case 1: f(1); break; case 2: h(1); break; case 3: f(2); break; default: h(2); } }
