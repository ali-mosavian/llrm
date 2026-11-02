/* Soft-float runtime: bit-level entry points and the RTLIB names built on them. */
#ifndef SOFTFLOAT_H
#define SOFTFLOAT_H

#include <stdint.h>

typedef uint32_t sf32;
typedef uint64_t sf64;

#define SF32_QNAN 0x7FC00000u
#define SF64_QNAN 0x7FF8000000000000ull

sf32 sf32_add(sf32 a, sf32 b);
sf32 sf32_sub(sf32 a, sf32 b);
sf32 sf32_mul(sf32 a, sf32 b);
sf32 sf32_div(sf32 a, sf32 b);
/* -1, 0, 1 ordered; 2 unordered. */
int sf32_cmp(sf32 a, sf32 b);

sf32 sf32_from_i32(int32_t x);
sf32 sf32_from_u32(uint32_t x);
sf32 sf32_from_i64(int64_t x);
sf32 sf32_from_u64(uint64_t x);
/* Toward zero; out of range saturates, NaN gives 0. */
int32_t sf32_to_i32(sf32 a);
uint32_t sf32_to_u32(sf32 a);
int64_t sf32_to_i64(sf32 a);
uint64_t sf32_to_u64(sf32 a);

sf64 sf32_to_sf64(sf32 a);
sf32 sf64_to_sf32(sf64 a);

#endif
