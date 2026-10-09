/* SINGLE and DOUBLE to text (QB rt/fout.asm B$FloatFOUTBX, and B$ASCRND from
   rt/ifout.asm). */
#include "fout.h"
#include "i8out.h"

enum { SINGLE_DIGITS = 7, DOUBLE_DIGITS = 16 };

/* B$ASCRND: round the digits to `want` places, half up, and drop the zeros
   that leaves at the end.  `exponent` is that of the last digit. */
static void round_digits(
    Decimal *d,
    int *exponent,
    word want
)
{
    word at;

    if (d->count > want) {
        char next = d->text[want];

        *exponent += d->count - want;
        d->count = want;
        if (next >= '5') {
            for (at = want; at; at--) {
                if (d->text[at - 1] < '9') {
                    d->text[at - 1]++;
                    d->count = at;
                    break;
                }
                (*exponent)++;
            }
            if (!at) {
                d->text[0] = '1';
                d->count = 1;
            }
        }
    }
    while (d->count > 1 && d->text[d->count - 1] == '0') {
        d->count--;
        (*exponent)++;
    }
}

static word put_digits(
    char *out,
    const char *digits,
    word count
)
{
    copy_bytes(out, digits, count);
    return count;
}

static word put_zeros(
    char *out,
    word count
)
{
    word at;

    for (at = 0; at < count; at++)
        out[at] = '0';
    return count;
}

/* 123.45, 1000, .005: the digits laid out without an exponent. */
static word plain(
    char *out,
    const Decimal *d,
    int exponent
)
{
    int left = exponent + d->count;
    word length = 0, used = 0;

    if (left > 0) {
        used = left < d->count ? left : d->count;
        length += put_digits(out, d->text, used);
        length += put_zeros(out + length, left - used);
        if (used == d->count)
            return length;
    } else {
        left = -left;
    }
    out[length++] = '.';
    if (used == 0)
        length += put_zeros(out + length, left);
    return length + put_digits(out + length, d->text + used, d->count - used);
}

/* 1.5E+20, 2D-08: the first digit, a point, the rest, and the exponent of
   the first digit with two places at least. */
static word scientific(
    char *out,
    const Decimal *d,
    int exponent,
    int is_double
)
{
    word length = 0;

    out[length++] = d->text[0];
    if (d->count > 1) {
        out[length++] = '.';
        length += put_digits(out + length, d->text + 1, d->count - 1);
        exponent += d->count - 1;
    }
    out[length++] = is_double ? 'D' : 'E';
    out[length++] = exponent < 0 ? '-' : '+';
    if (exponent < 0)
        exponent = -exponent;
    if (exponent >= 100)
        out[length++] = '0' + exponent / 100;
    out[length++] = '0' + exponent / 10 % 10;
    out[length++] = '0' + exponent % 10;
    return length;
}

word fout_real(
    double v,
    int is_double,
    char *out
)
{
    Decimal d;
    int limit = is_double ? DOUBLE_DIGITS : SINGLE_DIGITS, exponent;
    int magnitude;

    if (!i8_output(v, &d))
        qb_error(BE_OVERFLOW);
    exponent = d.exponent;
    if (!(d.count == 1 && d.text[0] == '0'))
        exponent -= d.count;
    round_digits(&d, &exponent, limit);
    out[0] = d.sign;
    magnitude = exponent < 0 ? -exponent : exponent;
    if (magnitude > limit || exponent + (int)d.count > limit)
        return 1 + scientific(out + 1, &d, exponent, is_double);
    return 1 + plain(out + 1, &d, exponent);
}
