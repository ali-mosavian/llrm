/* The libcall sites as llrm-cc emits them (the test's own main is gcc's: llrm-m68k-llc takes no inline asm). */
#include <stdint.h>
uint64_t f_udiv(uint64_t n, uint64_t d) { return n / d; }
uint64_t f_umod(uint64_t n, uint64_t d) { return n % d; }
int64_t f_div(int64_t n, int64_t d) { return n / d; }
int64_t f_mod(int64_t n, int64_t d) { return n % d; }
