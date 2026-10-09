/* RND and RANDOMIZE (QB rt/random.asm).  The generator is a linear congruential
   one on 24 bits, so a program sees the sequence QB gives it. */
#include "qb.h"

enum {
    MULTIPLIER = 16598013L,
    INCREMENT = 12820163L,
    SEED_MASK = 0xFFFFFFL,
    SEED_BITS = 24
};

static unsigned long seed = 0x50000L;
static float last;

/* The last number as a SINGLE in [0, 1), whose address RND returns. */
static float *current(void)
{
    last = (float)seed / (float)(1L << SEED_BITS);
    return &last;
}

static float *advance(void)
{
    seed = (seed * MULTIPLIER + INCREMENT) & SEED_MASK;
    return current();
}

/* B$RND0: RND with no argument. */
float *B_RND0(void)
{
    return advance();
}

/* B$RND1: RND(x).  Zero repeats the last number, a negative x reseeds from its
   bits (the top byte added into the low three), and the next number follows. */
float *B_RND1(float x)
{
    unsigned long bits;

    copy_bytes((char *)&bits, (const char *)&x, 4);
    if (bits << 1 == 0)
        return current();
    if (bits >> 31)
        seed = ((bits & SEED_MASK) + (bits >> SEED_BITS)) & SEED_MASK;
    return advance();
}

/* B$RNZP: RANDOMIZE with a DOUBLE; the two high words of it, xor-ed, become the
   middle word of the seed. */
void B_RNZP(double x)
{
    unsigned words[4];

    copy_bytes((char *)words, (const char *)&x, 8);
    seed = seed & 0xFF0000FFUL | (unsigned long)(words[2] ^ words[3]) << 8;
}
#pragma aux B_RND0 "B$RND0"
#pragma aux B_RND1 "B$RND1"
#pragma aux B_RNZP "B$RNZP"
