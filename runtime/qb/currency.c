/* CURRENCY as text: PRINT, WRITE, STR$, MKC$ and CVC. The value is an integer
   of ten-thousandths; PRINT and STR$ write it as a number is written, a sign
   column and the digits with a point and no trailing zeros. The PDS and
   VB-DOS runtimes' routines (B$P<terminator>CY, B$STCY, B$FMKC, B$FCVC) are
   only in their libraries; the format is read from their outputs. */
#include "fout.h"
#include "nhstutil.h"
#include "prnval.h"
#include "qb.h"

enum { SCALE = 10000, PLACES = 4 };

/* The sign column, then the digits of `v`: the whole part (none when it is
   zero but a fraction follows, as .5), and the fraction trimmed of trailing
   zeros; 0 for zero. */
static unsigned currency_text(long long v, char *out)
{
    unsigned long long magnitude = v < 0 ? 0ULL - (unsigned long long)v : (unsigned long long)v;
    unsigned long long whole = magnitude / SCALE;
    unsigned fraction = (unsigned)(magnitude % SCALE), length = 1, places = PLACES;
    char digits[20];
    int count = 0;

    out[0] = v < 0 ? '-' : ' ';
    if (whole == 0 && fraction == 0) {
        out[length++] = '0';
        return length;
    }
    while (whole) {
        digits[count++] = '0' + (int)(whole % 10);
        whole /= 10;
    }
    while (count)
        out[length++] = digits[--count];
    if (fraction) {
        while (fraction % 10 == 0) {
            fraction /= 10;
            places--;
        }
        out[length++] = '.';
        out += length;
        length += places;
        while (places--) {
            out[places] = '0' + (int)(fraction % 10);
            fraction /= 10;
        }
    }
    return length;
}

static void currency(long long v, enum Terminator end)
{
    char text[FOUT_MAX];

    if (using_ops) {
        using_ops->currency(v);
        using_item_end(end);
        return;
    }
    print_numeral(text, currency_text(v, text), end);
}

#pragma aux B_PCCY "B$PCCY"
#pragma aux B_PSCY "B$PSCY"
#pragma aux B_PECY "B$PECY"
#pragma aux B_STCY "B$STCY"
#pragma aux B_FMKC "B$FMKC"
#pragma aux B_FCVC "B$FCVC"

void B_PCCY(long long v)
{
    currency(v, COMMA);
}

void B_PSCY(long long v)
{
    currency(v, SEMI);
}

void B_PECY(long long v)
{
    currency(v, EOL);
}

/* B$STCY: STR$ of a CURRENCY, in a temporary. */
SD *B_STCY(long long v)
{
    char text[FOUT_MAX], *data;
    unsigned length = currency_text(v, text);
    SD *result = str_tmp(length, &data);

    copy_bytes(data, text, length);
    return result;
}

/* B$FMKC: MKC$, the eight bytes of the value. */
SD *B_FMKC(long long v)
{
    char *data;
    SD *result = str_tmp(8, &data);

    copy_bytes(data, (const char *)&v, 8);
    return result;
}

/* B$FCVC: CVC, the address of a value that stays until the next conversion; a
   shorter string is an Illegal function call. */
long long *B_FCVC(SD *sd)
{
    static long long value;

    if (sd->len < 8)
        qb_error(BE_ILLFUN);
    copy_bytes((char *)&value, sd->ptr, 8);
    str_tmp_free(sd);
    return &value;
}
