/* READ and RESTORE (QB rt/read.asm).

   The module's DATA lines are in its DGROUP, each a two-byte key, then the
   text of its items and a NUL, and after the last a key of 0xFFFF and the byte
   1.  The module's data area keeps where the next item is. */
#include "fin.h"
#include "module.h"
#include "nhstutil.h"

enum { END_OF_DATA = 1, KEY_BYTES = 2 };

/* The data area, with the read pointer at the first item if RESTORE has not
   put it anywhere (B$GETADDR). */
static ModuleData *reading(void)
{
    ModuleData *data = module_data();

    if (!data->restored) {
        data->data = module_word(OF_DS);
        data->restored = 1;
    }
    return data;
}

/* Puts the read pointer at the first DATA line. */
static void restore_first(void)
{
    ModuleData *data = module_data();

    data->data = module_word(OF_DS);
    data->restored = 1;
}

/* B$RSTB: RESTORE to the first DATA line whose key is `line` or more. */
void B_RSTB(unsigned line)
{
    const char *at = (const char *)module_word(OF_DS);

    restore_first();
    while (*(const unsigned *)(at - KEY_BYTES) < line) {
        while (*at++)
            ;
        at += KEY_BYTES;
    }
    module_data()->data = (word)at;
}

/* One item: the text it starts at (Out of DATA at the end of the list), and
   the data area to leave after it. */
static const char *item_start(ModuleData **data)
{
    const char *text;
    char first;

    *data = reading();
    text = (const char *)(*data)->data;
    first = *text;
    while (first == ' ' || first == '\t')
        first = *++text;
    if (first == END_OF_DATA)
        qb_error(BE_NODATA);
    return (const char *)(*data)->data;
}

/* Leaves the read pointer after the item, which must have ended in a comma or
   the end of its line; the end of a line also skips the next line's key. */
static void item_end(ModuleData *data, const char *cursor, char delimiter)
{
    if (delimiter != ',' && delimiter != 0)
        qb_error(BE_SYNTAX);
    if (delimiter == 0)
        cursor += KEY_BYTES;
    data->data = (word)cursor;
}

static void read_number(byte type, void __far *destination)
{
    ModuleData *data;
    const char *cursor = item_start(&data);
    FinValue value;
    char delimiter = fin_number(&cursor, type, &value);

    item_end(data, cursor, delimiter);
    if (type == VT_I2)
        *(int __far *)destination = value.integer;
    else if (type == VT_I4)
        *(long __far *)destination = value.long_integer;
    else if (type == VT_R4)
        *(float __far *)destination = value.single;
    else
        *(double __far *)destination = value.real;
}

/* B$RDSD: the item as a string, assigned to the descriptor `destination`. */
void B_RDSD(void __far *destination)
{
    ModuleData *data;
    const char *cursor = item_start(&data), *start;
    word length;
    char delimiter = fin_string(&cursor, &start, &length);
    char *text;
    SD *item;

    item_end(data, cursor, delimiter);
    item = str_tmp(length, &text);
    copy_bytes(text, start, length);
    str_assign(item, (SD *)(word)(unsigned long)destination);
}

void B_RDI2(int __far *destination)
{
    read_number(VT_I2, destination);
}

void B_RDI4(long __far *destination)
{
    read_number(VT_I4, destination);
}

void B_RDR4(float __far *destination)
{
    read_number(VT_R4, destination);
}

void B_RDR8(double __far *destination)
{
    read_number(VT_R8, destination);
}
#pragma aux B_RSTB "B$RSTB"
#pragma aux B_RDI2 "B$RDI2"
#pragma aux B_RDI4 "B$RDI4"
#pragma aux B_RDR4 "B$RDR4"
#pragma aux B_RDR8 "B$RDR8"
#pragma aux B_RDSD "B$RDSD"
