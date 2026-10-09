/* Unsigned integers of a few thousand bits, enough to hold any double scaled by
   a power of ten, for turning numbers into digits and digits into numbers
   exactly (i8out.c, fin.c).  Only the operations those need. */
#ifndef QB_BIGINT_H
#define QB_BIGINT_H

#include "qb.h"

/* A double scaled by 10^341 and shifted by 971 places takes about 1200 bits. */
enum { BIG_LIMBS = 84 };

typedef struct Big {
    u16 limb[BIG_LIMBS];   /* least significant first */
    unsigned used;         /* limbs in use; the rest are zero */
} Big;

void big_set(Big *b, unsigned long long v);
int big_is_zero(const Big *b);
unsigned big_bits(const Big *b);
unsigned long long big_low64(const Big *b);

void big_mul_small(Big *b, u16 factor);
/* b *= 10^n */
void big_mul_pow10(Big *b, unsigned n);
void big_shl(Big *b, unsigned bits);
/* Shifts right; false if bits were lost that were not zero. */
int big_shr(Big *b, unsigned bits);
/* Keeps the low `bits` bits and clears the rest. */
void big_keep_low(Big *b, unsigned bits);
/* b /= divisor, returning the remainder. */
u16 big_div_small(Big *b, u16 divisor);
/* a -= b, for a >= b. */
void big_sub(Big *a, const Big *b);
int big_cmp(const Big *a, const Big *b);

#endif
