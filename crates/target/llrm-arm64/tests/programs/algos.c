int printf(const char *, ...);
static void bubble(int *a, int n) {
  for (int i = 0; i < n; i++) for (int j = 0; j + 1 < n - i; j++) if (a[j] > a[j + 1]) { int t = a[j]; a[j] = a[j + 1]; a[j + 1] = t; }
}
static int sieve(int n) {
  static char composite[2000];
  int count = 0;
  for (int i = 2; i < n; i++) {
    if (!composite[i]) { count++; for (int j = i * 2; j < n; j += i) composite[j] = 1; }
  }
  return count;
}
static unsigned long collatz(unsigned long n) { unsigned long steps = 0; while (n != 1) { n = (n & 1) ? 3 * n + 1 : n / 2; steps++; } return steps; }
static int gcd(int a, int b) { while (b) { int t = a % b; a = b; b = t; } return a; }
int main(void) {
  int a[16] = {9, 3, 7, 1, 8, 2, 6, 4, 5, 0, 15, 11, 14, 10, 13, 12};
  bubble(a, 16);
  for (int i = 0; i < 16; i++) printf("%d ", a[i]);
  printf("\n%d %lu %d\n", sieve(2000), collatz(837799), gcd(1071, 462));
  unsigned h = 2166136261u;
  for (int i = 0; i < 1000; i++) h = (h ^ (unsigned)i) * 16777619u;
  printf("%u\n", h);
  return 0;
}
