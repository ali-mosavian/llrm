int printf(const char *, ...);
int g = 5;
int f(int a, int b) {
  int s = 0;
  for (int i = 0; i < a; i++) s += i * b;
  return s;
}
int main(void) {
  printf("%d\n", f(10, g));
  return 0;
}
