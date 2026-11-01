/* READ and RESTORE (QB rt/read.asm).

   The module's DATA lines are in its data, each a two-byte key, then the text
   of its items and a NUL, and after the last a key of 0xFFFF and the byte 1.
   The module's data area keeps where the next item is. */
#include "fin.h"
#include "rtinit.h"
#include "module.h"
#include "nhstutil.h"

enum { END_OF_DATA = 1, KEY_BYTES = 2 };

/* The data area, with the read pointer at the first item if RESTORE has not put
   it anywhere (B$GETADDR). */
static ModuleData *reading(void)
{
    ModuleData *data = module_data();

    if (!md_restored(data)) {
        md_set_cursor(data, module_first_data());
        md_set_restored(data);
    }
    return data;
}

/* Puts the read pointer at the first DATA line. */
static void restore_first(void)
{
    ModuleData *data = module_data();

    md_set_cursor(data, module_first_data());
    md_set_restored(data);
}

#pragma aux B_RSTB "B$RSTB"
#pragma aux B_RDI2 "B$RDI2"
#pragma aux B_RDI4 "B$RDI4"
#pragma aux B_RDR4 "B$RDR4"
#pragma aux B_RDR8 "B$RDR8"
#pragma aux B_RDSD "B$RDSD"
#pragma aux B_RDCY "B$RDCY"

/* B$RSTB: RESTORE to the first DATA line whose key is `line` or more (0 is the
   first line). */
void B_RSTB(unsigned short line)
{
    const char *at = module_first_data();

    restore_first();
    while (*(const u16 *)(at - KEY_BYTES) < line) {
        while (*at++)
            ;
        at += KEY_BYTES;
    }
    md_set_cursor(module_data(), at);
}

/* One item: the text it starts at (Out of DATA at the end of the list), and the
   data area to leave after it.  While an INPUT line is waiting the items are
   its, and there is no data area. */
static const char *item_start(ModuleData **data)
{
    const char *text;

    if (qb_input_file && !qb_input_line)
        qb_input_line = qb_input_file_line(qb_input_file);
    if (qb_input_line) {
        *data = NULL;
        return qb_input_line;
    }
    *data = reading();
    text = md_cursor(*data);
    while (*text == ' ' || *text == '\t')
        text++;
    if (*text == END_OF_DATA)
        qb_error(BE_NODATA);
    return md_cursor(*data);
}

/* Leaves the read pointer after the item, which must have ended in a comma or
   the end of its line; the end of a line also skips the next line's key. */
static void item_end(ModuleData *data, const char *cursor, char delimiter)
{
    if (!data) {
        qb_input_line = cursor;
        return;
    }
    if (delimiter != ',' && delimiter != 0)
        qb_error(BE_SYNTAX);
    if (delimiter == 0)
        cursor += KEY_BYTES;
    md_set_cursor(data, cursor);
}

/* The number of `type` at the read pointer, stored through `destination`. */
static void read_number(byte type, qb_data_ptr destination)
{
    ModuleData *data;
    const char *cursor = item_start(&data);
    FinValue value;
    char delimiter = fin_number(&cursor, type, &value);

    item_end(data, cursor, delimiter);
    if (type == VT_I2)
        *(short QB_FAR *)destination = (short)value.integer;
    else if (type == VT_I4)
        *(long QB_FAR *)destination = value.long_integer;
    else if (type == VT_R4)
        *(float QB_FAR *)destination = value.single;
    else
        *(double QB_FAR *)destination = value.real;
}

/* B$RDSD: the item as a string, assigned to the descriptor `destination`. The compiler passes a second word, the
   length of a fixed string (B$RDFS reads those; 0 here), which the caller removes. */
void B_RDSD(qb_data_ptr destination, short width)
{
    ModuleData *data;
    const char *cursor = item_start(&data), *start;
    unsigned length;
    char delimiter = fin_string(&cursor, &start, &length);
    char *text;
    SD *item;

    item_end(data, cursor, delimiter);
    item = str_tmp(length, &text);
    copy_bytes(text, start, length);
    str_assign(item, QB_NEAR_OF(destination));
}

void B_RDI2(qb_data_ptr destination)
{
    read_number(VT_I2, destination);
}

void B_RDI4(qb_data_ptr destination)
{
    read_number(VT_I4, destination);
}

void B_RDR4(qb_data_ptr destination)
{
    read_number(VT_R4, destination);
}

void B_RDR8(qb_data_ptr destination)
{
    read_number(VT_R8, destination);
}

enum { CURRENCY_SCALE = 10000, CURRENCY_PLACES = 4 };

/* The exact ten-thousandths of the plain decimal text at `text`, `[-+]digits[.digits]` up to a blank, comma or the
   end, rounded half away from zero at the fifth place; false for any other form. A magnitude past the type's is
   error 6. */
static int exact_currency(const char *text, long long *out)
{
    unsigned long long whole = 0, fraction = 0, limit;
    int negative = 0, places = 0, digits = 0, round_up = 0;

    while (*text == ' ' || *text == '\t')
        text++;
    if (*text == '-' || *text == '+')
        negative = *text++ == '-';
    for (; *text >= '0' && *text <= '9'; text++, digits++) {
        if (whole > 922337203685477ULL)
            return 0;
        whole = whole * 10 + (unsigned)(*text - '0');
    }
    if (*text == '.') {
        for (text++; *text >= '0' && *text <= '9'; text++, digits++) {
            if (places < CURRENCY_PLACES) {
                fraction = fraction * 10 + (unsigned)(*text - '0');
                places++;
            } else if (places++ == CURRENCY_PLACES) {
                round_up = *text >= '5';
            }
        }
    }
    if (!digits || !(*text == 0 || *text == ',' || *text == ' ' || *text == '\t' || *text == '\r' || *text == '\n'))
        return 0;
    for (; places < CURRENCY_PLACES; places++)
        fraction *= 10;
    whole = whole * CURRENCY_SCALE + fraction + (unsigned)round_up;
    limit = negative ? 9223372036854775808ULL : 9223372036854775807ULL;
    if (whole > limit)
        qb_error(BE_OVERFLOW);
    *out = negative ? (long long)(0ULL - whole) : (long long)whole;
    return 1;
}

/* B$RDCY: the item as a CURRENCY, stored through `destination`. A plain decimal is read exactly (a double cannot hold
   19 digits); an exponent or a type character goes through the double. */
void B_RDCY(qb_data_ptr destination)
{
    ModuleData *data;
    const char *cursor = item_start(&data), *begin = cursor;
    FinValue value;
    char delimiter = fin_number(&cursor, VT_R8, &value);
    long long cents;

    item_end(data, cursor, delimiter);
    if (!exact_currency(begin, &cents)) {
        double scaled = value.real * CURRENCY_SCALE;

        if (scaled >= 9223372036854775808.0 || scaled < -9223372036854775808.0)
            qb_error(BE_OVERFLOW);
        cents = (long long)(scaled < 0 ? scaled - 0.5 : scaled + 0.5);
    }
    *(long long QB_FAR *)destination = cents;
}
