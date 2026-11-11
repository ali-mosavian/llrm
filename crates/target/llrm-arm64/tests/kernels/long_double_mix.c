int printf(const char *, ...);
long double mix(int a, long double x, int b, long double y, int c) { return x * y + a - b + c; }
int main(void) {
  volatile long double x = 1.5L, y = 4.0L;
  printf("%d\n", (int)mix(7, x, 2, y, 100));
  return 0;
}
