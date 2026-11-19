/* mathops.h on a host: the math library's. */
#include <float.h>
#include <math.h>

#include "mathops.h"
#include "qb.h"

void qb_sincos(const double *angle, double *sine, double *cosine)
{
    *sine = sin(*angle);
    *cosine = cos(*angle);
}

static double power;

/* B$LPOW: `x ^ y` on a host, which has no x87 operands to give B$POW4. The
   errors are B$POW4's: 0 to a negative power and a negative base with a
   fractional exponent are 5; 6 is a result past the extended range, y * log2|x|
   at 32767.5 (a result past a DOUBLE's is infinity, not an error). */
double *B_LPOW(double x, double y)
{
    if ((x == 0 && y < 0) || (x < 0 && fmod(y, 1) != 0))
        qb_error(5);
    if (x != 0 && y * log2(x < 0 ? -x : x) >= 32767.5)
        qb_error(6);
    power = pow(x, y);
    return &power;
}

/* B$LFPC: a DOUBLE times 10000 as the x87 gives B$FPCY: the product rounded to the extended format's 64 bits, then to
   the nearest integer, ties to even. A NaN or a product past 64 bits is the integer indefinite, no error. 128-bit
   arithmetic is two halves, by pointer: a wasm32 build has no compiler-rt and passes no structure by value. */
typedef struct {
    unsigned long long hi, lo;
} wide;

static int wide_length(const wide *v)
{
    if (v->hi)
        return 128 - __builtin_clzll(v->hi);
    return v->lo ? 64 - __builtin_clzll(v->lo) : 0;
}

static int wide_bit(const wide *v, unsigned i)
{
    return (int)((i < 64 ? v->lo >> i : v->hi >> (i - 64)) & 1);
}

static int wide_low_set(const wide *v, unsigned n)
{
    if (n <= 64)
        return n && (v->lo & (n == 64 ? ~0ULL : (1ULL << n) - 1)) != 0;
    return v->lo != 0 || (v->hi & ((1ULL << (n - 64)) - 1)) != 0;
}

/* v >> by, rounded to nearest, ties to even; by in 1..127. */
static void wide_shr_even(wide *v, unsigned by)
{
    wide q;
    int up = wide_bit(v, by - 1) && (wide_low_set(v, by - 1) || wide_bit(v, by));

    if (by >= 64) {
        q.hi = 0;
        q.lo = v->hi >> (by - 64);
    } else {
        q.hi = v->hi >> by;
        q.lo = v->lo >> by | v->hi << (64 - by);
    }
    if (up && ++q.lo == 0)
        q.hi++;
    *v = q;
}

long long *B_LFPC(double x)
{
    static long long scaled;
    union {
        double d;
        unsigned long long u;
    } bits;
    unsigned long long mantissa, hi, lo;
    int exponent, length, negative;
    wide product;

    scaled = (long long)0x8000000000000000ULL;
    if (x != x || x - x != 0)
        return &scaled;
    bits.d = x;
    negative = (int)(bits.u >> 63);
    exponent = (int)((bits.u >> 52) & 0x7ff);
    mantissa = bits.u & ((1ULL << 52) - 1);
    if (exponent == 0)
        exponent = -1074;
    else {
        mantissa |= 1ULL << 52;
        exponent -= 1075;
    }
    /* mantissa * 10000 from 32-bit pieces: at most 67 bits */
    lo = (mantissa & 0xffffffffULL) * 10000;
    hi = (mantissa >> 32) * 10000 + (lo >> 32);
    product.hi = hi >> 32;
    product.lo = hi << 32 | (lo & 0xffffffffULL);
    length = wide_length(&product);
    if (length > 64) {
        wide_shr_even(&product, (unsigned)(length - 64));
        exponent += length - 64;
    }
    if (exponent >= 0) {
        if (exponent > 64 || wide_length(&product) + exponent > 63)
            return &scaled;
        product.lo <<= exponent;
    } else if (-exponent > 127)
        product.hi = product.lo = 0;
    else
        wide_shr_even(&product, (unsigned)-exponent);
    if (product.hi || product.lo >> 63)
        return &scaled;
    scaled = negative ? -(long long)product.lo : (long long)product.lo;
    return &scaled;
}
