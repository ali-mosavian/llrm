int printf(const char *, ...);
typedef unsigned long size_t;
static long big(int n) {
  char buf[5000];
  for (int i = 0; i < 5000; i++) buf[i] = (char)(i * n);
  long s = 0;
  for (int i = 0; i < 5000; i += 7) s += buf[i];
  return s;
}
int main(void) {
  printf("%ld %ld\n", big(3), big(-5));
  return 0;
}
