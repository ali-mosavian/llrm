int printf(const char *, ...);
static long sum10(long a, long b, long c, long d, long e, long f, long g, long h, long i, long j) {
  return a + 2 * b + 3 * c + 4 * d + 5 * e + 6 * f + 7 * g + 8 * h + 9 * i + 10 * j;
}
static void fill(int *p, int n) {
  for (int i = 0; i < n; i++) p[i] = 100 + i;
}
long (*volatile call10)(long, long, long, long, long, long, long, long, long, long) = sum10;
void (*volatile filler)(int *, int) = fill;
volatile int count = 8;
static long f(int n) {
  int *a = __builtin_alloca(n * sizeof(int));
  filler(a, n);
  long r = 0;
  if (n > 1) r = call10(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
  long s = 0;
  for (int i = 0; i < n; i++) s += a[i];
  return r + s;
}
int main(void) {
  printf("%ld\n", f(count));
  return 0;
}
