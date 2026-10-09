/* String functions (QB rt/strfcn.asm).  Every result is a temporary: an argument is never changed. */
#include "nhstutil.h"

enum { TRIM_LEFT = 1, TRIM_RIGHT = 2 };

/* TRIM: the argument itself when it is empty, else a temporary holding what the blanks leave. */
static SD *trim(
    SD *sd,
    byte sides)
{
    word first = 0, last = sd->len;

    if (sd->len == 0)
        return sd;
    if (sides & TRIM_LEFT)
        while (first < last && sd->ptr[first] == ' ')
            first++;
    if (sides & TRIM_RIGHT)
        while (last > first && sd->ptr[last - 1] == ' ')
            last--;
    return str_tmp_copy(sd, first, last - first);
}

SD *B_LTRM(SD *sd)
{
    return trim(sd, TRIM_LEFT);
}

SD *B_RTRM(SD *sd)
{
    return trim(sd, TRIM_RIGHT);
}

/* B$SPAC: SPACE$(n). */
SD *B_SPAC(int n)
{
    char *data;
    SD *result;
    int i;

    if (n < 0)
        qb_error(BE_ILLFUN);
    result = str_tmp(n, &data);
    for (i = 0; i < n; i++)
        data[i] = ' ';
    return result;
}

/* B$FLEN: LEN of a string; a temporary is consumed. */
int B_FLEN(SD *sd)
{
    int len = sd->len;

    str_tmp_free(sd);
    return len;
}
#pragma aux B_LTRM "B$LTRM"
#pragma aux B_RTRM "B$RTRM"
#pragma aux B_SPAC "B$SPAC"
#pragma aux B_FLEN "B$FLEN"
