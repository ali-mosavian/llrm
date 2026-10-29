#include <stdio.h>
#include <stdint.h>
int sum_to(int);
int masks(int, int);
int64_t clamp(int64_t, int64_t, int64_t);
int gcd(int, int);
int signs(int);
int shifts(int, int);
int logic(int, int);
int divs(int, int);
int64_t widen(int);
int64_t call_many(int64_t);
int64_t take_many(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, signed char, int);
int twice(int);
int indirect(int (*)(int), int);

int64_t c_many(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e, int64_t f, int64_t g, int64_t h, int64_t i, signed char j, int k) {
  return a + 2 * b + 3 * c + 4 * d + 5 * e + 6 * f + 7 * g + 8 * h + 9 * i + 10 * j + 11 * k;
}
int c_twice(int x) { return 2 * x + 1; }

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
  for (int i = 0; i < count; i++) {
    int64_t sx = (signed char)vals[i];
    int64_t wide = (sx + (int64_t)(unsigned char)vals[i] + vals[i] + (int64_t)(unsigned)vals[i]) * 8;
    if (widen(vals[i]) != wide) { printf("widen(%d)\n", vals[i]); bad++; }
    for (int j = 0; j < count; j++) {
      unsigned a = vals[i], b = vals[j], s = b & 31;
      unsigned want = (a << s) + (a >> s) * 3 + (unsigned)((int)a >> s) * 5;
      if ((unsigned)shifts(vals[i], vals[j]) != want) { printf("shifts(%d,%d)\n", vals[i], vals[j]); bad++; }
      unsigned mix = (a & b) + (a | 61680) + (a ^ b) + ~b + a * b + -a;
      if ((unsigned)logic(vals[i], vals[j]) != mix) { printf("logic(%d,%d)\n", vals[i], vals[j]); bad++; }
      if (vals[j] == 0 || (vals[i] == (int)0x80000000 && vals[j] == -1)) continue;
      unsigned d = (unsigned)(vals[i] / vals[j]) + a / b + (unsigned)(vals[i] % vals[j]);
      if ((unsigned)divs(vals[i], vals[j]) != d) { printf("divs(%d,%d)\n", vals[i], vals[j]); bad++; }
    }
  }
  for (int64_t a = -3; a < 5; a++) {
    int64_t sum = c_many(a, a + 1, a + 2, a + 3, a + 4, a + 5, a + 6, a + 7, a + 8, -3, 4);
    if (call_many(a) != sum + a) { printf("call_many(%ld)\n", (long)a); bad++; }
    int64_t want = (a + (a + 8)) * 3 + -5 + 7;
    if (take_many(a, 0, 0, 0, 0, 0, 0, 0, a + 8, -5, 7) != want) { printf("take_many(%ld)\n", (long)a); bad++; }
  }
  for (int x = -4; x < 9; x++) {
    int r = 2 * x + 1, s = 2 * r + 1;
    if (twice(x) != r + s + x) { printf("twice(%d)\n", x); bad++; }
    if (indirect(c_twice, x) != r) { printf("indirect(%d)\n", x); bad++; }
  }
  if (bad) printf("%d bad\n", bad);
  return bad != 0;
}
