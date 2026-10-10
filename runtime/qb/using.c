/* PRINT USING.

   The format is literal text and fields.  A string field is ! (the first
   character), & (all of it) or two backslashes with spaces between (as wide as
   that).  A numeric field is # for each digit, with an optional point, commas
   (every third digit), a leading + or $$ or ** or **$, a trailing + or -, or
   ^^^^ for an exponent; _ makes the next character literal.  The format is used
   again from its start for each item left over. */
#include "console.h"
#include "fout.h"
#include "prnval.h"

enum {
    FORMAT_MAX = 255,
    DIGITS_MAX = 80,
    NO_FIELD = 0
};

enum FieldKind { LITERAL, FIRST_CHARACTER, WHOLE_STRING, FIXED_STRING, NUMBER };

/* A numeric field as the format writes it. */
typedef struct NumberField {
    unsigned length;         /* characters of the format */
    unsigned width;          /* columns of the number, without the signs */
    int leading_plus;
    char trailing;           /* '+', '-' or 0 */
    int stars, dollar, commas;
    int before_point;        /* digit positions before the point */
    int decimals;
    int point;
    int carets;              /* 4 or 5 for an exponent, else 0 */
} NumberField;

/* A number as digits: value = 0.text * 10^integers, so integers is how many
   digits are before the point; zero or negative when the number is below 1. */
typedef struct Number {
    int negative;
    char text[DIGITS_MAX + 2];
    int count;
    int integers;
    int scientific;          /* PRINT would write it with an exponent */
} Number;

static char format[FORMAT_MAX];
static unsigned format_length, at;
static Number number;
static char body[FORMAT_MAX + DIGITS_MAX];   /* the number's own characters */
static char line[FORMAT_MAX + DIGITS_MAX + 8];   /* the whole field */

static void using_begin(SD *text)
{
    unsigned length = text->len;

    if (length > FORMAT_MAX)
        qb_error(BE_ILLFUN);
    copy_bytes(format, text->ptr, length);
    format_length = length;
    at = 0;
}

/* The numeric field that starts at `from`, if there is one. */
static int parse_number(unsigned from, NumberField *f)
{
    unsigned p = from;
    int digits_seen = 0;

    f->leading_plus = f->stars = f->dollar = f->commas = 0;
    f->before_point = f->decimals = f->point = f->carets = 0;
    f->trailing = 0;
    f->width = 0;
    if (format[p] == '+' && p + 1 < format_length) {
        f->leading_plus = 1;
        p++;
    }
    if (p + 1 < format_length && format[p] == '*' && format[p + 1] == '*') {
        f->stars = 1;
        f->width += 2;
        f->before_point += 2;
        p += 2;
        if (p < format_length && format[p] == '$') {
            f->dollar = 1;
            f->width++;
            p++;
        }
    } else if (p + 1 < format_length && format[p] == '$'
               && format[p + 1] == '$') {
        f->dollar = 1;
        f->width += 2;
        f->before_point++;
        p += 2;
    }
    while (p < format_length && (format[p] == '#' || format[p] == ',')) {
        if (format[p] == ',')
            f->commas = 1;
        else
            f->before_point++;
        f->width++;
        digits_seen = 1;
        p++;
    }
    if (p < format_length && format[p] == '.' && p + 1 < format_length
        && format[p + 1] == '#') {
        f->point = 1;
        f->width++;
        p++;
        while (p < format_length && format[p] == '#') {
            f->decimals++;
            f->width++;
            digits_seen = 1;
            p++;
        }
    } else if (p < format_length && format[p] == '.' && digits_seen) {
        f->point = 1;
        f->width++;
        p++;
    }
    if (!digits_seen && !f->stars && !f->dollar)
        return 0;
    if (p + 3 < format_length && format[p] == '^' && format[p + 1] == '^'
        && format[p + 2] == '^' && format[p + 3] == '^') {
        f->carets = 4;
        p += 4;
        if (p < format_length && format[p] == '^') {
            f->carets = 5;
            p++;
        }
    }
    if (p < format_length && (format[p] == '+' || format[p] == '-')
        && !f->leading_plus) {
        f->trailing = format[p++];
    }
    f->length = p - from;
    return 1;
}

/* The field that starts at `from`: its kind and how long it is in the format. f
   holds the numeric field. */
static enum FieldKind field_at(unsigned from, unsigned *length, NumberField *f)
{
    char c = format[from];
    unsigned p;

    if (c == '!') {
        *length = 1;
        return FIRST_CHARACTER;
    }
    if (c == '&') {
        *length = 1;
        return WHOLE_STRING;
    }
    if (c == '\\') {
        for (p = from + 1; p < format_length; p++) {
            if (format[p] == '\\') {
                *length = p - from + 1;
                return FIXED_STRING;
            }
            if (format[p] != ' ')
                break;
        }
        return LITERAL;
    }
    if ((c == '#' || c == '.' || c == '+' || c == '$' || c == '*')
        && parse_number(from, f)) {
        *length = f->length;
        return NUMBER;
    }
    return LITERAL;
}

/* Writes the literal text from `at` up to the next field, and says if a field
   follows. */
static int literal(void)
{
    unsigned length;
    NumberField scratch;

    while (at < format_length) {
        if (format[at] == '_' && at + 1 < format_length) {
            cn_putc(format[at + 1]);
            at += 2;
        } else if (field_at(at, &length, &scratch) != LITERAL) {
            return 1;
        } else {
            cn_putc(format[at++]);
        }
    }
    return 0;
}

/* Moves to the next field for an item, writing the text before it; a format
   without any field is an error. */
static enum FieldKind next_field(unsigned *length, NumberField *f)
{
    unsigned start = at;

    if (!literal()) {
        if (start == 0)
            qb_error(BE_ILLFUN);
        at = 0;
        if (!literal())
            qb_error(BE_ILLFUN);
    }
    return field_at(at, length, f);
}

static void using_end(int newline)
{
    literal();
    using_ops = 0;
    if (newline) {
        cn_crlf();
        cn_reset_output();
    }
}

/* Leading zeros so that every digit before the point is in the text. */
static void normalise(Number *n)
{
    int missing = -n->integers, i;

    if (missing <= 0)
        return;
    if (missing > DIGITS_MAX)
        missing = DIGITS_MAX;
    for (i = n->count + missing - 1; i >= missing; i--)
        n->text[i] = i - missing < n->count ? n->text[i - missing] : '0';
    for (i = 0; i < missing; i++)
        n->text[i] = '0';
    n->count += missing;
    n->integers = 0;
    if (n->count > DIGITS_MAX)
        n->count = DIGITS_MAX;
}

/* Rounds half up to `places` digits after the point, and pads with zeros to
   them. */
static void round_to(Number *n, int places)
{
    int keep, i;

    normalise(n);
    keep = n->integers + places;
    if (keep > DIGITS_MAX)
        keep = DIGITS_MAX;
    if (keep >= n->count) {
        while (n->count < keep)
            n->text[n->count++] = '0';
        return;
    }
    i = keep;
    n->count = keep;
    if (n->text[i] < '5')
        return;
    for (i = keep - 1; i >= 0; i--) {
        if (n->text[i] != '9') {
            n->text[i]++;
            return;
        }
        n->text[i] = '0';
    }
    for (i = n->count; i > 0; i--)
        n->text[i] = n->text[i - 1];
    n->text[0] = '1';
    n->count++;
    n->integers++;
}

static unsigned put(unsigned length, char c)
{
    body[length] = c;
    return length + 1;
}

/* The digits before the point, with a comma every third where the field has
   commas, at `length` in `line`; a lone zero where the number is below 1 and
   the field has a place for it. */
static unsigned put_integer(unsigned length, const NumberField *f)
{
    int first = 0, digits, i;

    while (first < number.integers && number.text[first] == '0')
        first++;
    digits = number.integers - first;
    if (digits == 0)
        return f->before_point > 0 ? put(length, '0') : length;
    for (i = 0; i < digits; i++) {
        if (f->commas && i > 0 && (digits - i) % 3 == 0)
            length = put(length, ',');
        length = put(length, number.text[first + i]);
    }
    return length;
}

static unsigned put_fraction(unsigned length, const NumberField *f)
{
    int i;

    if (!f->point)
        return length;
    length = put(length, '.');
    for (i = 0; i < f->decimals; i++)
        length = put(length, number.text[number.integers + i]);
    return length;
}

/* The sign characters a field asks for, the one before the number and the one
   after. */
static char leading_sign(const NumberField *f)
{
    return f->leading_plus ? (number.negative ? '-' : '+') : 0;
}

static char trailing_sign(const NumberField *f)
{
    if (f->trailing == '+')
        return number.negative ? '-' : '+';
    if (f->trailing == '-')
        return number.negative ? '-' : ' ';
    return 0;
}

/* Whether the characters of a number, with its sign and dollar, fit the columns
   of a field. */
static int fits_columns(const NumberField *f, unsigned length)
{
    int minus = number.negative && !f->leading_plus && !f->trailing;

    return length + (minus || f->leading_plus ? 1 : 0) + (f->dollar ? 1 : 0)
           <= f->width + f->leading_plus;
}

static void write_exponent_overflow(const NumberField *f);

/* Writes a number that has been given its digits as the field lays it out:
   right-aligned in the field's columns, the minus sign in the columns of the
   digits when the field has no sign of its own, and a % before the number when
   it does not fit. */
static void write_number(
    const NumberField *f,
    const char *mantissa,
    unsigned mantissa_length,
    unsigned columns
)
{
    char lead = leading_sign(f), trail = trailing_sign(f);
    int minus = number.negative && !lead && !trail;
    unsigned needed = mantissa_length + (minus || lead ? 1 : 0)
                    + (f->dollar ? 1 : 0);
    unsigned length = 0, pad = 0;
    char fill = f->stars ? '*' : ' ';

    if (needed > columns)
        line[length++] = '%';
    else
        pad = columns - needed;
    while (pad--)
        line[length++] = fill;
    if (lead)
        line[length++] = lead;
    if (minus)
        line[length++] = '-';
    if (f->dollar)
        line[length++] = '$';
    copy_bytes(line + length, mantissa, mantissa_length);
    length += mantissa_length;
    if (trail)
        line[length++] = trail;
    cn_write(line, length);
}

/* A number without an exponent. */
static void write_plain(const NumberField *f)
{
    unsigned length;

    round_to(&number, f->decimals);
    length = put_integer(0, f);
    length = put_fraction(length, f);
    if (number.scientific && !fits_columns(f, length)) {
        cn_putc('%');
        write_exponent_overflow(f);
        return;
    }
    write_number(f, body, length, f->width + f->leading_plus);
}

/* A number as digits and an exponent, as the field's places lay it out: the
   first place holds the sign (a 0 where nothing but the point follows), the
   rest the digits of the number, and the exponent's own places follow.  The
   exponent is chosen to fill the places before the point.  `carets` is 4 for
   two digits of exponent and 5 for three. */
static unsigned format_exponent(const NumberField *f, int carets)
{
    int integers = f->before_point - 1, places, first = 0, shift = 0, i;
    int exponent = 0;
    unsigned length = 0;

    if (integers < 0)
        integers = 0;
    places = integers + f->decimals;
    normalise(&number);
    while (first < number.count && number.text[first] == '0')
        first++;
    if (first < number.count) {
        exponent = number.integers - first;
        for (i = first; i < number.count; i++)
            number.text[i - first] = number.text[i];
        number.count -= first;
        number.integers = 0;
        round_to(&number, places);
        if (number.integers) {
            exponent++;
            number.integers = 0;
        }
        shift = exponent - integers;
    } else {
        for (i = 0; i < places; i++)
            number.text[i] = '0';
    }
    if (number.negative)
        body[length++] = '-';
    else
        body[length++] = integers == 0 && f->point ? '0' : ' ';
    for (i = 0; i < places; i++) {
        if (i == integers && f->point)
            body[length++] = '.';
        if (i < integers - 1 && number.text[i] == '0' && first == number.count)
            body[length++] = ' ';
        else
            body[length++] = number.text[i];
    }
    if (integers == places && f->point)
        body[length++] = '.';
    body[length++] = 'E';
    body[length++] = shift < 0 ? '-' : '+';
    if (shift < 0)
        shift = -shift;
    if (carets == 5)
        body[length++] = '0' + shift / 100 % 10;
    body[length++] = '0' + shift / 10 % 10;
    body[length++] = '0' + shift % 10;
    return length;
}

static void write_exponent(const NumberField *f)
{
    unsigned length = format_exponent(f, f->carets);
    char trail = trailing_sign(f);

    if (trail)
        body[length++] = trail;
    cn_write(body, length);
}

/* A number too big for its field and written with an exponent: the field's
   places before the point, two digits of exponent. */
static void write_exponent_overflow(const NumberField *f)
{
    unsigned length = format_exponent(f, 4);

    cn_write(body, length);
}

static void write_field(const NumberField *f)
{
    if (f->carets)
        write_exponent(f);
    else
        write_plain(f);
}

static void load_integer(long value)
{
    unsigned long magnitude = value < 0 ? 0UL - (unsigned long)value
                                        : (unsigned long)value;
    char reversed[12];
    int count = 0, i;

    number.negative = value < 0;
    number.scientific = 0;
    do {
        reversed[count++] = '0' + (int)(magnitude % 10);
        magnitude /= 10;
    } while (magnitude);
    for (i = 0; i < count; i++)
        number.text[i] = reversed[count - 1 - i];
    number.count = count;
    number.integers = count;
}

static void load_real(double value, int is_double)
{
    Decimal d;
    int exponent, limit = is_double ? 16 : 7;

    fout_digits(value, is_double, &d, &exponent);
    number.scientific = (exponent < 0 ? -exponent : exponent) > limit
                        || exponent + (int)d.count > limit;
    fout_digits(value, 1, &d, &exponent);
    number.negative = d.sign == '-';
    copy_bytes(number.text, d.text, d.count);
    number.count = d.count;
    number.integers = d.count + exponent;
}

/* The numeric field the next item goes to. */
static void number_item(void)
{
    unsigned length;
    NumberField f;

    if (next_field(&length, &f) != NUMBER)
        qb_error(BE_TYPE);
    at += length;
    write_field(&f);
}

static void using_integer(long value)
{
    load_integer(value);
    number_item();
}

static void using_real(double value, int is_double)
{
    load_real(value, is_double);
    number_item();
}

static void using_string(SD *item)
{
    unsigned length, width, shown;
    NumberField scratch;
    enum FieldKind kind = next_field(&length, &scratch);

    if (kind == NUMBER)
        qb_error(BE_TYPE);
    at += length;
    if (kind == FIRST_CHARACTER) {
        cn_putc(item->len ? item->ptr[0] : ' ');
    } else if (kind == WHOLE_STRING) {
        cn_write(item->ptr, item->len);
    } else {
        width = length;
        shown = item->len < width ? item->len : width;
        cn_write(item->ptr, shown);
        while (shown++ < width)
            cn_putc(' ');
    }
    str_tmp_free(item);
}

static const UsingOps ops = {
    using_integer, using_real, using_string, using_end
};

/* B$USNG: PRINT USING, with the format. */
void B_USNG(SD *format)
{
    using_begin(format);
    str_tmp_free(format);
    using_ops = &ops;
}
#pragma aux B_USNG "B$USNG"
