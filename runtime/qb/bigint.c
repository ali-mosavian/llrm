/* Big unsigned integers (see bigint.h). */
#include "bigint.h"

static void trim(Big *b)
{
    while (b->used && b->limb[b->used - 1] == 0)
        b->used--;
}

void big_set(Big *b, unsigned long long v)
{
    unsigned at;

    for (at = 0; at < BIG_LIMBS; at++)
        b->limb[at] = 0;
    for (at = 0; v; at++, v >>= 16)
        b->limb[at] = (u16)v;
    b->used = at;
}

int big_is_zero(const Big *b)
{
    return b->used == 0;
}

/* The position of the highest set bit, counting from 1; 0 for zero. */
unsigned big_bits(const Big *b)
{
    unsigned bits = 0;
    u16 top;

    if (!b->used)
        return 0;
    for (top = b->limb[b->used - 1]; top; top >>= 1)
        bits++;
    return (b->used - 1) * 16 + bits;
}

/* The low 64 bits. */
unsigned long long big_low64(const Big *b)
{
    unsigned long long v = 0;
    unsigned at;

    for (at = 4; at--;)
        v = v << 16 | (at < b->used ? b->limb[at] : 0);
    return v;
}

void big_mul_small(Big *b, u16 factor)
{
    unsigned long carry = 0;
    unsigned at;

    for (at = 0; at < b->used; at++) {
        carry += (unsigned long)b->limb[at] * factor;
        b->limb[at] = (u16)carry;
        carry >>= 16;
    }
    if (carry && b->used < BIG_LIMBS)
        b->limb[b->used++] = (u16)carry;
}

void big_mul_pow10(Big *b, unsigned n)
{
    for (; n >= 4; n -= 4)
        big_mul_small(b, 10000);
    while (n--)
        big_mul_small(b, 10);
}

void big_shl(Big *b, unsigned bits)
{
    unsigned whole = bits / 16, part = bits % 16, at;

    if (!b->used)
        return;
    for (at = b->used; at--;) {
        unsigned long moved = (unsigned long)b->limb[at] << part;

        if (at + whole + 1 < BIG_LIMBS)
            b->limb[at + whole + 1] |= (u16)(moved >> 16);
        if (at + whole < BIG_LIMBS)
            b->limb[at + whole] = (u16)moved;
    }
    for (at = 0; at < whole && at < BIG_LIMBS; at++)
        b->limb[at] = 0;
    b->used += whole + 1;
    if (b->used > BIG_LIMBS)
        b->used = BIG_LIMBS;
    trim(b);
}

int big_shr(Big *b, unsigned bits)
{
    unsigned whole = bits / 16, part = bits % 16, at;
    int exact = 1;

    for (at = 0; at < whole && at < b->used; at++)
        if (b->limb[at])
            exact = 0;
    if (part && whole < b->used && (b->limb[whole] & ((1u << part) - 1)))
        exact = 0;
    for (at = 0; at < b->used; at++) {
        unsigned long pair = at + whole < b->used ? b->limb[at + whole] : 0;

        if (at + whole + 1 < b->used)
            pair |= (unsigned long)b->limb[at + whole + 1] << 16;
        b->limb[at] = (u16)(pair >> part);
    }
    trim(b);
    return exact;
}

void big_keep_low(Big *b, unsigned bits)
{
    unsigned whole = bits / 16, at;

    if (whole >= b->used)
        return;
    b->limb[whole] &= (u16)((1u << (bits % 16)) - 1);
    for (at = whole + 1; at < b->used; at++)
        b->limb[at] = 0;
    b->used = whole + 1;
    trim(b);
}

u16 big_div_small(Big *b, u16 divisor)
{
    unsigned long rest = 0;
    unsigned at;

    for (at = b->used; at--;) {
        rest = rest << 16 | b->limb[at];
        b->limb[at] = (u16)(rest / divisor);
        rest %= divisor;
    }
    trim(b);
    return (u16)rest;
}

void big_sub(Big *a, const Big *b)
{
    long borrow = 0;
    unsigned at;

    for (at = 0; at < a->used; at++) {
        long other = at < b->used ? b->limb[at] : 0;
        long diff = (long)a->limb[at] - other - borrow;

        borrow = diff < 0;
        a->limb[at] = (u16)(diff + (borrow ? 0x10000L : 0));
    }
    trim(a);
}

int big_cmp(const Big *a, const Big *b)
{
    unsigned at;

    if (a->used != b->used)
        return a->used < b->used ? -1 : 1;
    for (at = a->used; at--;)
        if (a->limb[at] != b->limb[at])
            return a->limb[at] < b->limb[at] ? -1 : 1;
    return 0;
}
