int printf(const char *, ...);
static double poly(double x) { return ((3.0 * x + 2.0) * x - 5.0) * x + 0.5; }
static float fsum(const float *p, int n) { float s = 0; for (int i = 0; i < n; i++) s += p[i] * 0.5f; return s; }
static int cmp(double a, double b) { return a < b ? -1 : a > b ? 1 : 0; }
int main(void) {
  double x = 1.25;
  for (int i = 0; i < 6; i++) {
    double y = poly(x + i);
    printf("%d %d\n", (int)(y * 100), (int)y);
  }
  float f[8];
  for (int i = 0; i < 8; i++) f[i] = (float)i * 1.5f;
  printf("%d\n", (int)(fsum(f, 8) * 10));
  printf("%d %d %d\n", cmp(1.0, 2.0), cmp(2.0, 1.0), cmp(1.0, 1.0));
  long big = 123456789012345L;
  double d = (double)big;
  printf("%ld %d\n", (long)(d / 3.0), (int)(float)(d / 1000000000.0) % 1000);
  unsigned long u = 18000000000000000000UL;
  printf("%lu\n", (unsigned long)(double)u / 1000);
  return 0;
}
