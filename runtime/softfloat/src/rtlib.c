/* RTLIB names over the bit-level entry points. */
#include "softfloat.h"


static inline sf32 f2b(float f) { sf32 b; __builtin_memcpy(&b, &f, 4); return b; }
static inline float b2f(sf32 b) { float f; __builtin_memcpy(&f, &b, 4); return f; }
static inline sf64 d2b(double f) { sf64 b; __builtin_memcpy(&b, &f, 8); return b; }
static inline double b2d(sf64 b) { double f; __builtin_memcpy(&f, &b, 8); return f; }

float __addsf3(float a, float b) { return b2f(sf32_add(f2b(a), f2b(b))); }
float __subsf3(float a, float b) { return b2f(sf32_sub(f2b(a), f2b(b))); }
float __mulsf3(float a, float b) { return b2f(sf32_mul(f2b(a), f2b(b))); }
float __divsf3(float a, float b) { return b2f(sf32_div(f2b(a), f2b(b))); }
float __negsf2(float a) { return b2f(f2b(a) ^ 0x80000000u); }

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. */
int __cmpsf2(float a, float b) { int c = sf32_cmp(f2b(a), f2b(b)); return c == 2 ? 1 : c; }
int __eqsf2(float a, float b) { return __cmpsf2(a, b); }
int __nesf2(float a, float b) { return __cmpsf2(a, b); }
int __ltsf2(float a, float b) { return __cmpsf2(a, b); }
int __lesf2(float a, float b) { return __cmpsf2(a, b); }
int __gesf2(float a, float b) { int c = sf32_cmp(f2b(a), f2b(b)); return c == 2 ? -1 : c; }
int __gtsf2(float a, float b) { return __gesf2(a, b); }
int __unordsf2(float a, float b) { return sf32_cmp(f2b(a), f2b(b)) == 2; }

float __floatsisf(int32_t x) { return b2f(sf32_from_i32(x)); }
float __floatunsisf(uint32_t x) { return b2f(sf32_from_u32(x)); }
float __floatdisf(int64_t x) { return b2f(sf32_from_i64(x)); }
float __floatundisf(uint64_t x) { return b2f(sf32_from_u64(x)); }
int32_t __fixsfsi(float a) { return sf32_to_i32(f2b(a)); }
uint32_t __fixunssfsi(float a) { return sf32_to_u32(f2b(a)); }
int64_t __fixsfdi(float a) { return sf32_to_i64(f2b(a)); }
uint64_t __fixunssfdi(float a) { return sf32_to_u64(f2b(a)); }
double __extendsfdf2(float a) { return b2d(sf32_to_sf64(f2b(a))); }
float __truncdfsf2(double a) { return b2f(sf64_to_sf32(d2b(a))); }
