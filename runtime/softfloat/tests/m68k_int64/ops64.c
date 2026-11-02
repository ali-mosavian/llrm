#include <stdint.h>
uint64_t sf_o_shl(uint64_t x, int n) { return x << n; }
uint64_t sf_o_shr(uint64_t x, int n) { return x >> n; }
uint64_t sf_o_add(uint64_t x, uint64_t y) { return x + y; }
uint64_t sf_o_sub(uint64_t x, uint64_t y) { return x - y; }
uint64_t sf_o_mul(uint64_t x, uint64_t y) { return x * y; }
uint64_t sf_o_mulw(uint32_t x, uint32_t y) { return (uint64_t)x * y; }
uint64_t sf_o_div(uint64_t x, uint64_t y) { return x / y; }
uint64_t sf_o_mod(uint64_t x, uint64_t y) { return x % y; }
uint64_t sf_o_sticky(uint64_t x, int n) { return (x >> n) | ((x << (64 - n)) != 0); }
int sf_o_lt(uint64_t x, uint64_t y) { return x < y; }
