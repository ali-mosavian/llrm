/* Soft-float runtime. Every entry point carries its canonical compiler-rt/libgcc name
 * (`__addsf3`, ...); SF_NAME(n) is `__n` in the runtime and `sfb_n` in the host test. */
#ifndef SOFTFLOAT_H
#define SOFTFLOAT_H

#include <stdint.h>

#ifndef SF_NAME
#define SF_CAT(a, b) a##b
#define SF_NAME(n) SF_CAT(__, n)
#endif

typedef uint32_t sf32;
typedef uint64_t sf64;

#define SF32_QNAN 0x7FC00000u

/* SF_INTABI: entry points take and return the bit patterns as integers, for targets whose backend has no
 * float type yet. The soft-float calling convention passes a float exactly like a same-width integer. */
#ifdef SF_INTABI
typedef uint32_t sf_float;
typedef uint64_t sf_double;
static inline sf32 f2b(sf_float f) { return f; }
static inline sf_float b2f(sf32 b) { return b; }
static inline sf64 d2b(sf_double f) { return f; }
static inline sf_double b2d(sf64 b) { return b; }
#else
typedef float sf_float;
typedef double sf_double;
static inline sf32 f2b(float f) { union { float f; sf32 b; } u; u.f = f; return u.b; }
static inline float b2f(sf32 b) { union { float f; sf32 b; } u; u.b = b; return u.f; }
static inline sf64 d2b(double f) { union { double f; sf64 b; } u; u.f = f; return u.b; }
static inline double b2d(sf64 b) { union { double f; sf64 b; } u; u.b = b; return u.f; }
#endif

/* Round and pack: m has three guard bits and its implicit one at bit 26; e may be <= 0 or >= 255. */
sf32 sf32_pack(uint32_t sign, int e, uint32_t m);

#endif
