/* String functions (QB rt/strfcn.asm).  Every result is a temporary: an
   argument is never changed. */
#include "nhstutil.h"

enum { TRIM_LEFT = 1, TRIM_RIGHT = 2 };

/* TRIM: the argument itself when it is empty, else a temporary holding what the
   blanks leave. */
static SD *trim(SD *sd, byte sides)
{
    unsigned first = 0, last = sd->len;

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

/* B$SCPY: a copy of a string in a temporary. */
SD *B_SCPY(SD *sd)
{
    return str_tmp_copy(sd, 0, sd->len);
}

/* B$SCPF: a function's string result, copied to a temporary before the frame
   that owns it goes; the original is deleted. */
SD *B_SCPF(SD *sd)
{
    SD *copy = str_tmp_copy(sd, 0, sd->len);

    str_release(sd);
    sd->len = 0;
    return copy;
}

/* UCASE$ and LCASE$: the argument's letters in one case, in a temporary. */
static SD *recase(SD *sd, char low, char high, int shift)
{
    char *data;
    unsigned at;
    SD *result = str_tmp(sd->len, &data);

    for (at = 0; at < sd->len; at++) {
        char c = sd->ptr[at];

        data[at] = c >= low && c <= high ? c + shift : c;
    }
    str_tmp_free(sd);
    return result;
}

SD *B_UCAS(SD *sd)
{
    return recase(sd, 'a', 'z', 'A' - 'a');
}

SD *B_LCAS(SD *sd)
{
    return recase(sd, 'A', 'Z', 'a' - 'A');
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
#pragma aux B_SCPF "B$SCPF"
#pragma aux B_UCAS "B$UCAS"
#pragma aux B_LCAS "B$LCAS"
#pragma aux B_SCPY "B$SCPY"
