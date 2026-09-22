/* The freestanding modern-language runtime. Built through qbopt's C frontend. */

typedef signed char i8;
typedef unsigned char u8;
typedef int i16;
typedef unsigned int u16;
typedef unsigned long u32;
typedef long i32;

extern void rt_write(const char *text, u16 length);
extern int main(void);
static char fixed_buffer[48];

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

void pt(const char *text)
{
    rt_write(text, text_length(text));
}

void pn(void)
{
    rt_write("\r\n", 2);
}

static char *unsigned_decimal(char *after, u32 value)
{
    do {
        *--after = (char)('0' + value % 10);
        value /= 10;
    } while (value != 0);
    return after;
}

static void decimal(u32 magnitude, u8 negative)
{
    char *after = fixed_buffer + 16;
    char *text = unsigned_decimal(after, magnitude);
    if (negative)
        *--text = '-';
    rt_write(text, (u16)(after - text));
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
    rt_write(fixed_buffer, (u16)(out - fixed_buffer));
}
