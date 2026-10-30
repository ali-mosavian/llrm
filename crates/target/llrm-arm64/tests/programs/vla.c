int printf(const char *, ...);
typedef unsigned long size_t;
static int vla(int n) {
  int a[n];
  for (int i = 0; i < n; i++) a[i] = i * i;
  int s = 0;
  for (int i = 0; i < n; i++) s += a[i];
  return s;
}
static long nested(int n) {
  long total = 0;
  for (int k = 1; k <= n; k++) {
    char buffer[k * 3];
    for (int i = 0; i < k * 3; i++) buffer[i] = (char)(i + k);
    total += buffer[k * 3 - 1] + vla(k);
  }
  return total;
}
static int many(int n, long a, long b, long c, long d, long e, long f, long g, long h, long i, long j) {
  int pad[n];
  pad[0] = (int)(a + b + c + d + e + f + g + h + i + j);
  return pad[0] + n;
}
int main(void) {
  printf("%d %d %ld\n", vla(5), vla(50), nested(6));
  printf("%d\n", many(3, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
  return 0;
}
