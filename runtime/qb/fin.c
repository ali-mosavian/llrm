/* Text to numbers and strings (QB rt/fin.asm, which hands the digits to the
   math pack's $i8_input; the scaling here is the one i8out.c ports). */
#include "fin.h"
#include "i8out.h"

enum { MAX_EXPONENT = 308 };

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

/* The value as a double: the mantissa scaled by its power of ten in extended
   precision, then rounded to a double as it is stored. */
static double to_real(const Parsed *parsed)
{
    unsigned long long mantissa = parsed->mantissa;
    long double x;

    if (mantissa == 0)
        return 0;
    if (parsed->exponent > MAX_EXPONENT + 20)
        qb_error(BE_OVERFLOW);
    /* a 64-bit mantissa above 2^63 is converted in two halves, exactly */
    x = (long double)(long long)(mantissa >> 1) * 2;
    x += (long double)(int)(mantissa & 1);
    x = i8_scale(x, parsed->exponent);
    if (x > 1.7976931348623157e308L)
        qb_error(BE_OVERFLOW);
    return parsed->negative ? -(double)x : (double)x;
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
