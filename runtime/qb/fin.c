/* Text to numbers and strings (QB rt/fin.asm, which hands the digits to the
   math pack's $i8_input; the scaling here is the one i8out.c ports). */
#include "fin.h"
#include "bigint.h"

/* The most a 64-bit mantissa takes another digit at. */
#define MANTISSA_LIMIT 1844674407370955160ULL

static int is_blank(char c)
{
    return c == ' ' || c == '\t' || c == '\n';
}

static char upper(char c)
{
    return c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c;
}

/* B$GETCH: the next character that is not a blank, in capitals, and the cursor
   after it. */
static char next_char(const char **cursor)
{
    char c;

    do
        c = *(*cursor)++;
    while (is_blank(c));
    return upper(c);
}

static const char *skip_blanks(const char *text)
{
    while (is_blank(*text))
        text++;
    return text;
}

static int digit_value(char c)
{
    c = upper(c);
    if (c >= '0' && c <= '9')
        return c - '0';
    return c >= 'A' && c <= 'F' ? c - 'A' + 10 : 99;
}

/* What the digits of one number came to. */
typedef struct Parsed {
    unsigned long long mantissa;   /* the leading digits that fit 64 bits */
    int exponent;                  /* of ten, for the mantissa */
    int negative;
    int whole;                     /* an integer, with nothing left over */
} Parsed;

/* &H and &O numbers, which are 16 or 32 bits wide, and whose high half is the
   sign bit (&HFFFF is -1). */
static char radix_number(
    const char **cursor,
    int radix,
    byte type,
    FinValue *value
)
{
    const char *p = *cursor;
    unsigned long total = 0;

    while (digit_value(*p) < radix) {
        if (total > 0xFFFFFFFFUL / radix)
            qb_error(BE_OVERFLOW);
        total = total * radix + digit_value(*p++);
    }
    if (type == VT_I2) {
        if (total > 0xFFFF)
            qb_error(BE_OVERFLOW);
        value->integer = (int)total;
    } else if (type == VT_I4) {
        value->long_integer = (long)total;
    } else {
        double real = total > 0xFFFF ? (double)(long)total : (double)(int)total;

        if (type == VT_R4)
            value->single = real;
        else
            value->real = real;
    }
    *cursor = p;
    return next_char(cursor);
}

/* Digits, a point, an exponent: the decimal number at `p`. */
static const char *decimal(const char *p, Parsed *out)
{
    int seen_point = 0, exponent = 0, sign = 1;
    const char *mark;

    out->mantissa = 0;
    out->exponent = 0;
    out->negative = 0;
    out->whole = 1;
    if (*p == '-' || *p == '+') {
        out->negative = *p == '-';
        p++;
    }
    for (;; p++) {
        if (*p >= '0' && *p <= '9') {
            if (out->mantissa < MANTISSA_LIMIT) {
                out->mantissa = out->mantissa * 10 + (*p - '0');
                if (seen_point)
                    out->exponent--;
            } else if (!seen_point) {
                out->exponent++;
                out->whole = 0;
            }
        } else if (*p == '.' && !seen_point) {
            seen_point = 1;
            out->whole = 0;
        } else {
            break;
        }
    }
    mark = p;
    if (upper(*p) == 'E' || upper(*p) == 'D') {
        p++;
        if (*p == '-' || *p == '+')
            sign = *p++ == '-' ? -1 : 1;
        if (*p >= '0' && *p <= '9') {
            while (*p >= '0' && *p <= '9' && exponent < 10000)
                exponent = exponent * 10 + (*p++ - '0');
            while (*p >= '0' && *p <= '9')
                p++;
            out->exponent += sign * exponent;
            out->whole = 0;
        } else {
            p = mark;
        }
    }
    return p;
}

/* Working numbers for the conversion below, off the small stack. */
static Big numerator, divisor, shifted;

/* floor(numerator * 2^scale / divisor), 63 or 64 bits, by long division; the
   scale is chosen to make it so, and *inexact says if there was a remainder. */
static unsigned long long divide_scaled(int *scale, int *inexact)
{
    enum { QUOTIENT_BITS = 63 };
    unsigned long long quotient = 0;
    unsigned at;

    *scale = QUOTIENT_BITS + (int)big_bits(&divisor) - (int)big_bits(&numerator);
    if (*scale < 0)
        *scale = 0;
    big_shl(&numerator, (unsigned)*scale);
    for (at = 64; at--;) {
        shifted = divisor;
        big_shl(&shifted, at);
        if (big_cmp(&numerator, &shifted) >= 0) {
            big_sub(&numerator, &shifted);
            quotient |= 1ULL << at;
        }
    }
    *inexact = !big_is_zero(&numerator);
    return quotient;
}

/* The double nearest `mantissa` * 10^`exponent`, a tie going to the even one,
   built from its bits.  A value too big for a double is an Overflow, and one
   too small is a denormal or zero. */
static double nearest_double(unsigned long long mantissa, int exponent)
{
    enum { PRECISION = 53, HIDDEN = 52, LOWEST = -1074, BIAS = 1075 };
    unsigned long long kept, bits;
    int scale = 0, inexact = 0, lowest, cut;
    unsigned length;
    double result;

    big_set(&numerator, mantissa);
    if (exponent >= 0) {
        big_mul_pow10(&numerator, (unsigned)exponent);
    } else {
        big_set(&divisor, 1);
        big_mul_pow10(&divisor, (unsigned)-exponent);
        big_set(&shifted, divide_scaled(&scale, &inexact));
        numerator = shifted;
    }
    /* the value is numerator * 2^-scale, and a little more if inexact; keep 53
       bits of it, or fewer for a denormal */
    length = big_bits(&numerator);
    lowest = -scale;
    cut = length > PRECISION ? (int)length - PRECISION : 0;
    if (lowest + cut < LOWEST)
        cut = LOWEST - lowest;
    if (cut > 0) {
        int roundbit, lost_bits_zero;

        shifted = numerator;
        lost_bits_zero = big_shr(&shifted, (unsigned)cut - 1);
        roundbit = (int)(big_low64(&shifted) & 1);
        kept = big_low64(&shifted) >> 1;
        if (roundbit && (!lost_bits_zero || inexact || (kept & 1)))
            kept++;
    } else {
        /* a short number is shifted up to put its top bit at the hidden one, as
           far as the lowest denormal allows */
        int grow = PRECISION - (int)length;

        if (grow > lowest - LOWEST)
            grow = lowest - LOWEST;
        kept = big_low64(&numerator) << grow;
        lowest -= grow;
    }
    lowest += cut > 0 ? cut : 0;
    if (kept >> PRECISION) {
        kept >>= 1;
        lowest++;
    }
    bits = kept;
    if (kept >> HIDDEN) {
        if (lowest + BIAS >= 2047)
            qb_error(BE_OVERFLOW);
        bits = (unsigned long long)(lowest + BIAS) << HIDDEN;
        bits |= kept & ((1ULL << HIDDEN) - 1);
    }
    copy_bytes((char *)&result, (const char *)&bits, 8);
    return result;
}

/* The parsed number as a double. */
static double to_real(const Parsed *parsed)
{
    double magnitude = nearest_double(parsed->mantissa, parsed->exponent);

    return parsed->negative ? -magnitude : magnitude;
}

/* B$ftolrnd: to the nearest integer, a half going to the even one. */
static long round_to_long(double real)
{
    long whole = (long)real;
    double fraction = real - whole;

    if (fraction > 0.5 || (fraction == 0.5 && (whole & 1)))
        return whole + 1;
    if (fraction < -0.5 || (fraction == -0.5 && (whole & 1)))
        return whole - 1;
    return whole;
}

char fin_number(const char **cursor, byte type, FinValue *value)
{
    const char *p = skip_blanks(*cursor);
    Parsed parsed;
    double real;
    long whole;
    char suffix;

    if (*p == '&') {
        char radix = upper(p[1]);

        *cursor = p + (radix == 'H' || radix == 'O' ? 2 : 1);
        return radix_number(cursor, radix == 'H' ? 16 : 8, type, value);
    }
    p = decimal(p, &parsed);
    suffix = *p;
    if (suffix == '%' || suffix == '&' || suffix == '!' || suffix == '#')
        p++;
    *cursor = p;
    real = to_real(&parsed);
    if (type == VT_R8) {
        value->real = real;
    } else if (type == VT_R4) {
        value->single = (float)real;
    } else {
        whole = parsed.whole && parsed.mantissa <= 0x7FFFFFFFUL
            ? (parsed.negative ? -(long)parsed.mantissa : (long)parsed.mantissa)
            : round_to_long(real);
        if (type == VT_I2) {
            if (whole < -32768L || whole > 32767L)
                qb_error(BE_OVERFLOW);
            value->integer = (int)whole;
        } else {
            value->long_integer = whole;
        }
        if (suffix == '%' && (whole < -32768L || whole > 32767L))
            qb_error(BE_TYPE);
    }
    return next_char(cursor);
}

char fin_string(const char **cursor, const char **start, unsigned *length)
{
    const char *p = skip_blanks(*cursor);
    char end = ',';

    if (*p == '"') {
        end = '"';
        p++;
    }
    *start = p;
    while (*p && *p != end)
        p++;
    *length = p - *start;
    if (end == ',') {
        while (*length && (*start)[*length - 1] == ' ')
            --*length;
    } else if (*p == '"') {
        p++;
    }
    *cursor = p;
    return next_char(cursor);
}
