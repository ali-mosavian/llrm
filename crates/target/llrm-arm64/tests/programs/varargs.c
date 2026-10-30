int printf(const char *, ...);
typedef __builtin_va_list va_list;
#define va_start(a, b) __builtin_va_start(a, b)
#define va_arg(a, t) __builtin_va_arg(a, t)
#define va_end(a) __builtin_va_end(a)
static long sum(int n, ...) {
  va_list ap;
  va_start(ap, n);
  long s = 0;
  for (int i = 0; i < n; i++) s += va_arg(ap, int);
  va_end(ap);
  return s;
}
static double mix(int n, ...) {
  va_list ap;
  va_start(ap, n);
  double s = 0;
  for (int i = 0; i < n; i++) { s += va_arg(ap, double); s += va_arg(ap, long); }
  va_end(ap);
  return s;
}
static int fmt(char *out, const char *f, ...) {
  va_list ap;
  va_start(ap, f);
  int n = 0;
  for (; *f; f++) {
    if (*f == 'd') { int v = va_arg(ap, int); out[n++] = (char)('0' + v % 10); }
    else if (*f == 's') { const char *s = va_arg(ap, const char *); while (*s) out[n++] = *s++; }
  }
  out[n] = 0;
  va_end(ap);
  return n;
}
int main(void) {
  printf("%ld %ld\n", sum(3, 1, 2, 3), sum(10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
  printf("%d\n", (int)mix(3, 1.5, 10L, 2.5, 20L, 3.5, 30L));
  char buffer[64];
  int n = fmt(buffer, "dsdsd", 4, "ab", 5, "cd", 6);
  printf("%d %s\n", n, buffer);
  return 0;
}
