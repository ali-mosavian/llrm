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
double fops(double, double);
int fcmps(double, double);
double fmax_(double, double) __asm__("_fmaxd");
double convert(int, int64_t, float);
double sum_array(const double *, int);
void scale(float *, float);
double fcalls(double, int);
int fbits(float);

double c_fmix(int n, double a, float f, int64_t i, double d, double e, double g, double h, double j, double k, double l, float m, signed char c) {
  return n + 2 * a + 3 * f + 4 * i + 5 * d + 6 * e + 7 * g + 8 * h + 9 * j + 10 * k + 11 * l + 12 * m + 13 * c;
}
float c_half(float x) { return x * 0.5f; }

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
  {
    double fv[] = {0.0, 1.5, -2.25, 1e10, -3.0, 0.1};
    int nf = sizeof fv / sizeof *fv;
    double nan = __builtin_nan("");
    for (int i = 0; i < nf + 1; i++)
      for (int j = 0; j < nf + 1; j++) {
        double a = i < nf ? fv[i] : nan, b = j < nf ? fv[j] : nan;
        if (j < nf && fops(a, fv[j] == 0 ? 1 : b) != -(((a + (fv[j] == 0 ? 1 : b)) * (a - (fv[j] == 0 ? 1 : b))) / (fv[j] == 0 ? 1 : b)) && a == a) { printf("fops(%g,%g)\n", a, b); bad++; }
        int want = (a < b) | (a <= b) << 1 | (a > b) << 2 | (a >= b) << 3 | (a == b) << 4 | (a < b || a > b) << 5 |
                   (a == b || a != a || b != b) << 6 | (a != b) << 7 | (a != a || b != b) << 8 | (a == a && b == b) << 9 |
                   (!(a >= b)) << 10 | (!(a < b)) << 11;
        if (fcmps(a, b) != want) { printf("fcmps(%g,%g)=%x want %x\n", a, b, fcmps(a, b), want); bad++; }
        double m = a > b ? a : b;
        double got = fmax_(a, b);
        if (!(got == m || (got != got && m != m))) { printf("fmax(%g,%g)\n", a, b); bad++; }
      }
    float fl = 2.5f;
    for (int a = -5; a < 6; a++) {
      double c = convert(a, 1000000007LL * a, fl);
      float t = (float)((double)a + (double)(uint64_t)(1000000007LL * a) + (double)fl);
      if (c != (double)(int)(double)t) { printf("convert(%d)=%g\n", a, c); bad++; }
    }
    double arr[] = {1.5, 2.5, -4.0, 8.25, 0.125};
    if (sum_array(arr, 5) != 8.375) { printf("sum_array\n"); bad++; }
    float cell = 3.0f;
    scale(&cell, 1.5f);
    if (cell != 4.5f) { printf("scale\n"); bad++; }
    for (int n = -3; n < 4; n++) {
      double a = 0.75 * n;
      float g = c_half((float)n);
      double r = c_fmix(n, a, g, 7, a, a, a, a, a, a, a, g, 5);
      if (fcalls(a, n) != r + a) { printf("fcalls(%d)\n", n); bad++; }
    }
    float bf[] = {0.0f, 1.0f, -2.5f};
    for (int i = 0; i < 3; i++) {
      int bits; __builtin_memcpy(&bits, &bf[i], 4);
      if (fbits(bf[i]) != bits) { printf("fbits\n"); bad++; }
    }
  }
  if (bad) printf("%d bad\n", bad);
  return bad != 0;
}
