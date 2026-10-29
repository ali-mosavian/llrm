#include <stdio.h>
#include <stdint.h>
int sum_to(int);
int masks(int, int);
int64_t clamp(int64_t, int64_t, int64_t);
int gcd(int, int);
int signs(int);

static int ref_masks(int a, int b) {
  unsigned ua = a, ub = b;
  return 3 * (a == b) + 5 * (a != b) + 6 * (a < b) + 7 * (a <= b) + 9 * (a > b) + 10 * (a >= b) + 11 * (ua < ub) + 12 * (ua <= ub) + 13 * (ua > ub) + 14 * (ua >= ub);
}

int main(void) {
  int bad = 0;
  for (int n = 1; n < 50; n++) {
    int want = n * (n - 1) / 2;
    if (sum_to(n) != want) { printf("sum_to(%d)=%d want %d\n", n, sum_to(n), want); bad++; }
  }
  int vals[] = {0, 1, -1, 2, -2, 4096, 4097, -4096, -4097, 0x7fffffff, (int)0x80000000, 100, 101};
  int count = sizeof vals / sizeof *vals;
  for (int i = 0; i < count; i++)
    for (int j = 0; j < count; j++)
      if (masks(vals[i], vals[j]) != ref_masks(vals[i], vals[j])) {
        printf("masks(%d,%d)=%x want %x\n", vals[i], vals[j], masks(vals[i], vals[j]), ref_masks(vals[i], vals[j]));
        bad++;
      }
  int64_t big[] = {0, 5, -5, 1000000007LL, -1000000007LL};
  for (int i = 0; i < 5; i++)
    for (int64_t lo = -10; lo < 10; lo += 7)
      for (int64_t hi = lo; hi < lo + 30; hi += 11) {
        int64_t want = big[i] < lo ? lo : big[i];
        want = want > hi ? hi : want;
        if (clamp(big[i], lo, hi) != want) { printf("clamp bad\n"); bad++; }
      }
  for (int x = 1; x < 40; x++)
    for (int y = 1; y < 40; y++) {
      int a = x, b = y;
      while (a != b) { if ((unsigned)a > (unsigned)b) a -= b; else b -= a; }
      if (gcd(x, y) != a) { printf("gcd(%d,%d)=%d want %d\n", x, y, gcd(x, y), a); bad++; }
    }
  for (int x = -3; x < 200; x++) {
    int want = x < 0 ? -1 : (unsigned)x > 100 ? 2 : 1;
    if (signs(x) != want) { printf("signs(%d)\n", x); bad++; }
  }
  if (bad) printf("%d bad\n", bad);
  return bad != 0;
}
