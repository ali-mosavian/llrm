/* Benchmark kernels: the same C goes to cc65 and, through clang's IR, to llrm-6502.
   `buf` is 16 bytes defined by the driver: 3 1 4 1 5 9 2 6 5 3 5 8 9 7 9 3. */
extern unsigned char buf[];

#ifdef K_zero
unsigned char zero(void) { return 0; }
#endif
#ifdef K_add8
unsigned char add8(unsigned char a, unsigned char b) { return a + b; }
#endif
#ifdef K_add16
unsigned add16(unsigned a, unsigned b) { return a + b; }
#endif
#ifdef K_max8
unsigned char max8(unsigned char a, unsigned char b) { return a > b ? a : b; }
#endif
#ifdef K_max16
unsigned max16(unsigned a, unsigned b) { return a > b ? a : b; }
#endif
#ifdef K_sum_arr
unsigned char sum_arr(unsigned char n) { unsigned char s = 0, i; for (i = 0; i < n; ++i) s += buf[i]; return s; }
#endif
#ifdef K_find
unsigned char find(unsigned char c) { unsigned char i = 0; while (buf[i] != c) ++i; return i; }
#endif
#ifdef K_fill
unsigned char fill(unsigned char n) { unsigned char i; for (i = 0; i < n; ++i) buf[i] = i; return buf[n - 1]; }
#endif
#ifdef K_popcount
unsigned char popcount(unsigned char x) { unsigned char c = 0; while (x) { c += x & 1; x >>= 1; } return c; }
#endif
#ifdef K_mul8
unsigned char mul8(unsigned char a, unsigned char b) { return a * b; }
#endif
#ifdef K_fib
unsigned fib(unsigned char n) { unsigned a = 0, b = 1, t; while (n--) { t = a + b; a = b; b = t; } return a; }
#endif
#ifdef K_gcd
unsigned char gcd(unsigned char a, unsigned char b) { while (a != b) { if (a > b) a -= b; else b -= a; } return a; }
#endif
#ifdef K_sum4
unsigned char sum4(void) { unsigned char s = 0, i; for (i = 0; i < 4; ++i) s += buf[i]; return s; }
#endif

/* The second set: library-style routines, not seen while the backend was tuned.
   `str` is "hello world", `tab` is 2 5 8 11 14 17 20 23. */
extern char str[];
extern unsigned char tab[];

#ifdef K_slen
unsigned slen(const char *s) { const char *p = s; while (*p) ++p; return p - s; }
#endif
#ifdef K_scmp
int scmp(const char *a, const char *b) { while (*a && *a == *b) { ++a; ++b; } return (unsigned char)*a - (unsigned char)*b; }
#endif
#ifdef K_mcopy
unsigned char mcopy(unsigned char *d, const unsigned char *s, unsigned char n) { unsigned char i; for (i = 0; i < n; ++i) d[i] = s[i]; return d[n - 1]; }
#endif
#ifdef K_crc16
unsigned crc16(const unsigned char *p, unsigned char n) {
  unsigned c = 0xFFFF; unsigned char i, b;
  for (i = 0; i < n; ++i) { c ^= (unsigned)p[i] << 8; for (b = 0; b < 8; ++b) c = (c & 0x8000) ? (c << 1) ^ 0x1021 : c << 1; }
  return c;
}
#endif
#ifdef K_mul16
unsigned mul16(unsigned a, unsigned b) { return a * b; }
#endif
#ifdef K_digits
unsigned char digits(unsigned v) { unsigned char n = 1; while (v >= 10) { v /= 10; ++n; } return n; }
#endif
#ifdef K_reverse
unsigned char reverse(unsigned char *p, unsigned char n) {
  unsigned char i = 0, j = n - 1, t;
  while (i < j) { t = p[i]; p[i] = p[j]; p[j] = t; ++i; --j; }
  return p[0];
}
#endif
#ifdef K_bsearch8
unsigned char bsearch8(const unsigned char *t, unsigned char n, unsigned char key) {
  unsigned char lo = 0, hi = n, mid;
  while (lo < hi) { mid = (lo + hi) >> 1; if (t[mid] < key) lo = mid + 1; else hi = mid; }
  return lo;
}
#endif
#ifdef K_sum16
unsigned sum16(const unsigned char *p, unsigned char n) { unsigned s = 0; unsigned char i; for (i = 0; i < n; ++i) s += p[i]; return s; }
#endif
#ifdef K_schr
unsigned char schr(const char *s, unsigned char c) { const char *p = s; while (*p && *p != (char)c) ++p; return *p ? p - s : 255; }
#endif
#ifdef K_lz4
/* The C of cc65's libsrc/common/lz4.s (a162565e), with its memcpy as forward byte loops. */
unsigned lz4(const unsigned char *in, unsigned char *out, unsigned outlen) {
  unsigned char token, tmp;
  unsigned offset;
  unsigned char *start = out, *end = out + outlen, *src;
  while (out < end) {
    token = *in++;
    offset = token >> 4;
    token &= 0x0f;
    token += 4;
    if (offset == 15) {
      do { tmp = *in++; offset += tmp; } while (tmp == 255);
    }
    while (offset) { *out++ = *in++; --offset; }
    if (out >= end) break;
    offset = *in++;
    tmp = *in++;
    offset += (unsigned)tmp << 8;
    src = out - offset;
    offset = token;
    if (token == 19) {
      do { tmp = *in++; offset += tmp; } while (tmp == 255);
    }
    while (offset) { *out++ = *src++; --offset; }
  }
  return out - start;
}
#endif
/* Loops that keep several variables live across the iteration. */
#ifdef K_xs8
unsigned char xs8(unsigned char x, unsigned char n) { while (n--) { x ^= x << 3; x ^= x >> 5; x ^= x << 1; } return x; }
#endif
#ifdef K_minmax
unsigned char minmax(const unsigned char *p, unsigned char n) {
  unsigned char lo = 255, hi = 0, i;
  for (i = 0; i < n; ++i) { unsigned char v = p[i]; if (v < lo) lo = v; if (v > hi) hi = v; }
  return hi - lo;
}
#endif
#ifdef K_fletcher
unsigned fletcher(const unsigned char *p, unsigned char n) {
  unsigned char a = 0, b = 0, i;
  for (i = 0; i < n; ++i) { a += p[i]; if (a >= 255) a -= 255; b += a; if (b >= 255) b -= 255; }
  return ((unsigned)b << 8) | a;
}
#endif
#ifdef K_hash16
unsigned hash16(const char *s) { unsigned h = 5381; while (*s) { h = (h << 5) + h + (unsigned char)*s; ++s; } return h; }
#endif
#ifdef K_dotx
unsigned char dotx(const unsigned char *a, const unsigned char *b, unsigned char n) {
  unsigned char s = 0, i;
  for (i = 0; i < n; ++i) s += a[i] ^ b[i];
  return s;
}
#endif
