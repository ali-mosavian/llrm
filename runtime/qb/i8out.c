/* Binary to decimal digits: the sixteen leading digits of a double, rounded the
   way QB's PRINT and STR$ round them.

   The digits are exact, from big-integer arithmetic on the double's mantissa
   and exponent, with one measured quirk: QB does not round up at one half but
   when the part beyond the sixteenth digit reaches 1 - 922 * 10^16 / 2^64
   (0.50018...), as if it added a bias of 922 / 2^64 before cutting the digits
   off. */
#include "bigint.h"
#include "i8out.h"

enum { DIGITS = 16 };

/* 2^64 - 922 * 10^16: the fraction, in units of 2^-64, from which the last
   digit rounds up. */
#define ROUND_UP_FROM 9226744073709551616ULL

#define TEN_15 1000000000000000ULL
#define TEN_16 10000000000000000ULL

/* Working numbers: kept off the stack, which is small, and used by one
   conversion at a time. */
#define number big_w[0]
#define rest big_w[1]
#define edge big_w[2]
#define whole big_w[3]

/* The pieces of a finite double: value = mantissa * 2^exponent. */
typedef struct Parts {
    unsigned long long mantissa;
    int exponent;
    int negative;
} Parts;

static void split(unsigned long long bits, Parts *parts)
{
    unsigned biased = (unsigned)(bits >> 52) & 0x7FF;

    parts->negative = (int)(bits >> 63);
    parts->mantissa = bits & 0xFFFFFFFFFFFFFULL;
    if (biased) {
        parts->mantissa |= 1ULL << 52;
        parts->exponent = (int)biased - 1075;
    } else {
        parts->exponent = -1074;
    }
}

/* A first guess at k, where 10^(k-1) <= value < 10^k: from the length of the
   mantissa and log10(2); scaled() says if it is one off. */
static int guess_exponent(const Parts *parts)
{
    unsigned long long m = parts->mantissa;
    int length = 0;

    for (; m; m >>= 1)
        length++;
    return (int)(((long)(parts->exponent + length - 1) * 1233L) >> 12) + 1;
}

/* The magnitude of the value being converted, for the FPU's quick way. */
static long double magnitude;

/* llrm-c puts a long double local at the wrong place in the frame, so the FPU's
   working numbers are statics. */
static long double ten_power, product, integral, threshold, difference;

/* x87's extended numbers hold 64 bits, enough for value * 10^s to be rounded
   only once and a little (relative 2^-64, under 0.0006 of the last digit) when
   10^s is exact, which it is up to 10^27.  The digits then come out of the
   FPU's product, unless the part beyond them is so close to the rounding
   threshold that the error could change the answer; those, and the numbers
   outside that range, go on to digits_for.  Returns 0 for "not decided". */
static int quick_digits(int k, unsigned long long *digits, int *up)
{
    enum { MOST = 27 };
    int s = DIGITS - k, at;

    if (s < 0 || s > MOST)
        return 0;
    ten_power = 1;
    for (at = 0; at < s; at++)
        ten_power *= 10;
    product = magnitude * ten_power;
    *digits = (unsigned long long)product;
    integral = (long double)(long long)*digits;
    threshold = (long double)ROUND_UP_FROM / 18446744073709551616.0L;
    difference = product - integral - threshold;
    if (difference < 0.001L && difference > -0.001L)
        return 0;
    *up = difference > 0;
    return 1;
}

/* The sixteen digits of value * 10^(16-k), as a number, cut off and not yet
   rounded, and whether rounding adds one.  They are 16 digits only when k is
   right; otherwise one short of 10^15, or 10^16 and over, and the caller
   corrects k. */
static unsigned long long digits_for(const Parts *parts, int k, int *up)
{
    int s = DIGITS - k;
    unsigned long long quick;

    *up = 0;
    if (quick_digits(k, &quick, up))
        return quick;
    big_set(&number, parts->mantissa);
    if (s >= 0) {
        big_mul_pow10(&number, (unsigned)s);
        if (parts->exponent >= 0) {
            big_shl(&number, (unsigned)parts->exponent);
            return big_low64(&number);
        }
        /* a fraction of 2^-lost: the whole part, and what is left over */
        {
            unsigned lost = (unsigned)-parts->exponent;

            rest = number;
            big_keep_low(&rest, lost);
            big_shr(&number, lost);
            big_shl(&rest, 64);
            big_set(&edge, ROUND_UP_FROM);
            big_shl(&edge, lost);
        }
    } else {
        /* 10^16 or more is a whole number: cut -s digits off it */
        unsigned cut = (unsigned)-s, at;

        big_shl(&number, (unsigned)parts->exponent);
        whole = number;
        for (at = cut; at >= 4; at -= 4)
            big_div_small(&whole, 10000);
        for (; at; at--)
            big_div_small(&whole, 10);
        rest = whole;
        big_mul_pow10(&rest, cut);
        big_sub(&number, &rest);
        rest = number;
        number = whole;
        big_shl(&rest, 64);
        big_set(&edge, ROUND_UP_FROM);
        big_mul_pow10(&edge, cut);
    }
    *up = big_cmp(&rest, &edge) >= 0;
    return big_low64(&number);
}

/* Infinities and the not-numbers, which QB writes as text. */
static int special(unsigned long long bits, Decimal *out)
{
    const char *name;

    if (((bits >> 52) & 0x7FF) != 0x7FF)
        return 0;
    if (bits << 12 == 0)
        name = "1#INF";
    else if (bits == 0xFFF8000000000000ULL)
        name = "1#IND";
    else
        name = "1#NAN";
    copy_bytes(out->text, name, 5);
    out->count = 5;
    out->exponent = 1;
    return 1;
}

void i8_output(double value, Decimal *out)
{
    unsigned long long bits, q;
    Parts parts;
    int k, at, up;
    unsigned count = DIGITS;

    copy_bytes((char *)&bits, (const char *)&value, 8);
    magnitude = value < 0 ? -value : value;
    out->sign = bits >> 63 ? '-' : ' ';
    if (special(bits, out))
        return;
    split(bits, &parts);
    if (parts.mantissa == 0) {
        out->sign = ' ';
        out->text[0] = '0';
        out->count = 1;
        out->exponent = 0;
        return;
    }
    k = guess_exponent(&parts);
    for (;;) {
        q = digits_for(&parts, k, &up);
        if (q < TEN_15)
            k--;
        else if (q >= TEN_16)
            k++;
        else
            break;
    }
    q += up;
    if (q == TEN_16) {
        q = TEN_15;
        k++;
    }
    for (at = DIGITS; at--; q /= 10)
        out->text[at] = '0' + (int)(q % 10);
    while (count > 1 && out->text[count - 1] == '0')
        count--;
    out->count = count;
    out->exponent = k;
}
