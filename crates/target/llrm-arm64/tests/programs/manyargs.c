int printf(const char *, ...);
static long f(long a, long b, long c, long d, long e, long f, long g, long h, long i, long j, int k, char l) {
  return a + 2 * b + 3 * c + 4 * d + 5 * e + 6 * f + 7 * g + 8 * h + 9 * i + 10 * j + 11 * k + 12 * l;
}
static double g(double a, double b, double c, double d, double e, double f, double g, double h, double i, double j) {
  return a + b * 2 + c * 3 + d * 4 + e * 5 + f * 6 + g * 7 + h * 8 + i * 9 + j * 10;
}
static int apply(int (*fn)(int, int), int a, int b) { return fn(a, b); }
static int plus(int a, int b) { return a + b; }
static int times(int a, int b) { return a * b; }
int main(void) {
  printf("%ld\n", f(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12));
  printf("%d\n", (int)g(1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
  printf("%d %d\n", apply(plus, 6, 7), apply(times, 6, 7));
  printf("%d %ld %c %s %u %x\n", -5, 1234567890123L, 'q', "str", 4000000000u, 0xbeef);
  return 0;
}
