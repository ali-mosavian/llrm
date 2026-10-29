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
