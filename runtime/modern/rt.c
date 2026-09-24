/* The freestanding modern-language runtime: entry, panics, and formatting.
 * Built through qbopt's C frontend. */

#include "types.h"

extern int main(void);
extern char *rt_append_bytes(char *text, const char *bytes, u16 count);
static char fixed_buffer[48];

/* The field the next formatted value fills, which `rt_field` sets. */
static u8 field_width;
static u8 field_radix = 10;
static char field_fill = ' ';
static u8 field_left;

/* Where formatted text goes: the console, or while an f-string builds a
 * value, that string. One set of formatters serves both. */
static char *sink;

static void put(const char *text, u16 length)
{
    if (sink != 0)
        sink = rt_append_bytes(sink, text, length);
    else
        rt_write(text, length);
}

static void fill(u16 count)
{
    while (count-- != 0)
        put(&field_fill, 1);
}

/* Fills before a value of `length` bytes; what is owed after it. */
static u16 open_field(u16 length)
{
    u16 missing = field_width > length ? field_width - length : 0;
    field_width = 0;
    field_radix = 10;
    if (!field_left)
        fill(missing);
    return missing;
}

/* Fills after the value, and resets the field. */
static void close_field(u16 missing)
{
    if (field_left)
        fill(missing);
    field_fill = ' ';
    field_left = 0;
}

/* One formatted value, filled out to the field. A zero fill goes after
 * the sign. */
static void emit(const char *text, u16 length)
{
    u16 missing;
    if (field_fill == '0' && !field_left && length != 0 && *text == '-') {
        put(text++, 1);
        --length;
        if (field_width != 0)
            --field_width;
    }
    missing = open_field(length);
    put(text, length);
    close_field(missing);
}

void rt_field(u8 width, u8 radix, u8 filler, u8 left)
{
    field_width = width;
    field_radix = radix;
    field_fill = (char)filler;
    field_left = left;
}

void rt_begin(void)
{
    sink = rt_alloc(16, 1);
}

char *rt_end(void)
{
    char *built = sink;
    sink = 0;
    return built;
}

int start(void)
{
    return main();
}

static u16 text_length(const char *text)
{
    u16 length = 0;
    while (text[length] != 0)
        ++length;
    return length;
}

void pt(char *text)
{
    emit(text, *length_of(text));
}

/* A `&string` view, copied near in pieces the size of the buffer. */
void pv(const char far *data, u16 length)
{
    u16 missing = open_field(length);
    while (length != 0) {
        u16 piece = length < sizeof fixed_buffer ? length : sizeof fixed_buffer;
        u16 at;
        for (at = 0; at < piece; ++at)
            fixed_buffer[at] = data[at];
        put(fixed_buffer, piece);
        data += piece;
        length -= piece;
    }
    close_field(missing);
}

/* INT 0: division by zero, or a quotient too wide for its register. */
void rt_panic_divide(void)
{
    rt_panic("division by zero or overflow");
}

void rt_panic_convert(void)
{
    rt_panic("float outside the integer type");
}

void rt_panic_shift(void)
{
    rt_panic("shift count out of range");
}

void rt_panic_bounds(void)
{
    rt_panic("index out of bounds");
}

void pn(void)
{
    put("\r\n", 2);
}

void rt_panic(const char *message)
{
    sink = 0;
    rt_write("panic: ", 7);
    rt_write(message, text_length(message));
    rt_write("\r\n", 2);
    rt_exit(255);
}

static char *unsigned_decimal(char *after, u32 value)
{
    do {
        *--after = (char)('0' + value % 10);
        value /= 10;
    } while (value != 0);
    return after;
}

static char *unsigned_digits(char *after, u32 value, u8 radix)
{
    do {
        *--after = "0123456789abcdef"[value % radix];
        value /= radix;
    } while (value != 0);
    return after;
}

static void decimal(u32 magnitude, u8 negative)
{
    char *after = fixed_buffer + 16;
    char *text = unsigned_digits(after, magnitude, field_radix);
    if (negative)
        *--text = '-';
    emit(text, (u16)(after - text));
}

void pi1(i8 value) { decimal(value < 0 ? 0UL - (u32)(i32)value : (u32)value, value < 0); }
void pu1(u8 value) { decimal(value, 0); }
void pi2(i16 value) { decimal(value < 0 ? 0UL - (u32)(i32)value : (u32)value, value < 0); }
void pu2(u16 value) { decimal(value, 0); }
void pi4(i32 value) { decimal(value < 0 ? 0UL - (u32)value : (u32)value, value < 0); }
void pu4(u32 value) { decimal(value, 0); }

void pf4(i32 raw, u8 fraction)
{
    char *out = fixed_buffer;
    char *integer_after = fixed_buffer + 16;
    char *integer;
    u32 magnitude;
    u32 scale;
    u32 remainder;

    if (raw < 0) {
        *out++ = '-';
        magnitude = 0UL - (u32)raw;
    } else {
        magnitude = (u32)raw;
    }

    scale = 1UL << fraction;
    integer = unsigned_decimal(integer_after, magnitude >> fraction);
    while (integer != integer_after)
        *out++ = *integer++;
    *out++ = '.';

    remainder = magnitude & (scale - 1);
    if (remainder == 0) {
        *out++ = '0';
    } else {
        /* floor(remainder * 10 / scale), without a wider integer type.
         * scale is a power of two no larger than 2^31. Splitting it into
         * q*10+s keeps every intermediate in u32 even for Q1.31. */
        u32 q = scale / 10;
        u8 s = (u8)(scale % 10);
        do {
            u8 digit = 0;
            u8 candidate;
            for (candidate = 1; candidate <= 9; ++candidate) {
                u32 threshold = (u32)candidate * q
                    + ((u16)candidate * s + 9) / 10;
                if (remainder < threshold)
                    break;
                digit = candidate;
            }
            remainder = (remainder - (u32)digit * q) * 10
                - (u16)digit * s;
            *out++ = (char)('0' + digit);
        } while (remainder != 0);
    }
    emit(fixed_buffer, (u16)(out - fixed_buffer));
}

void pf2(i16 raw, u8 fraction) { pf4(raw, fraction); }

void pb(u8 value)
{
    if (value != 0)
        emit("true", 4);
    else
        emit("false", 5);
}

void pc(char value)
{
    fixed_buffer[0] = value;
    emit(fixed_buffer, 1);
}
