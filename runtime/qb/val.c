/* VAL (QB rt/fin.asm B$FVAL). */
#include "fin.h"
#include "nhstutil.h"

static double value;

/* B$FVAL: the number a string starts with, 0 if none, as a DOUBLE whose address
   is returned.  Blanks inside it do not count (VAL("3 4") is 34).  The parser
   needs a NUL, which the argument does not have, so it reads a copy with the
   blanks left out. */
double *B_FVAL(SD *sd)
{
    FinValue number;
    const char *cursor;
    char *text;
    SD *copy = str_tmp(sd->len + 1, &text);
    unsigned at, kept = 0;

    for (at = 0; at < sd->len; at++)
        if (sd->ptr[at] != ' ')
            text[kept++] = sd->ptr[at];
    text[kept] = '\0';
    cursor = text;
    fin_number(&cursor, VT_R8, &number);
    value = number.real;
    str_tmp_free(copy);
    str_tmp_free(sd);
    return &value;
}
#pragma aux B_FVAL "B$FVAL"
