int printf(const char *, ...);
#define N 32
static long double a[N], b[N];
int main(void) {
  for (int i = 0; i < N; i++) { a[i] = i + 0.5L; b[i] = 1.0L / (i + 1); }
  long double s = 0;
  for (int i = 0; i < N; i++) s += a[i] * b[i] - b[i] / (a[i] + 1);
  printf("%d %d\n", (int)s, (int)(s * 1000) % 1000);
  return 0;
}
