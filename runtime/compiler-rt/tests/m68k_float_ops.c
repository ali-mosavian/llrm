/* Plain float C compiled by llrm-cc (SoftenFloat turns each operator into a call of the library): the compiler-to-library contract at m68k.
 * tests/m68k_main.c calls these in place of the canonical names when M68K_FLOAT_C is set (tests/run_m68k.sh). */
#include <stdint.h>

float f_add(float x, float y) { return x + y; }
float f_sub(float x, float y) { return x - y; }
float f_mul(float x, float y) { return x * y; }
float f_div(float x, float y) { return x / y; }
int f_cmp(float x, float y) { return x != x || y != y ? 2 : x < y ? -1 : x > y ? 1 : 0; }
float f_i2f(int32_t n) { return (float)n; }
float f_u2f(uint32_t n) { return (float)n; }
int32_t f_f2i(float x) { return (int32_t)x; }
uint32_t f_f2u(float x) { return (uint32_t)x; }
