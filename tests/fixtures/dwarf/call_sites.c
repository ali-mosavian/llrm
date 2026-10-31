extern int g(int, int);
extern int h(int);
int f(int a, int b) {
  int r = g(a, 7);
  return h(r + b) + r;
}
