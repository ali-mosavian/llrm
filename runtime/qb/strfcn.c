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

/* A temporary of `n` copies of a character. */
static SD *repeated(int n, char c)
{
    char *data;
    SD *result;
    int i;

    if (n < 0)
        qb_error(BE_ILLFUN);
    result = str_tmp(n, &data);
    for (i = 0; i < n; i++)
        data[i] = c;
    return result;
}

/* B$SPAC: SPACE$(n). */
SD *B_SPAC(int n)
{
    return repeated(n, ' ');
}

/* B$STRI: STRING$(n, code). */
SD *B_STRI(int n, int code)
{
    return repeated(n, (char)code);
}

/* B$STRS: STRING$(n, text), the first character of the text. */
SD *B_STRS(int n, SD *text)
{
    char first;

    if (text->len == 0)
        qb_error(BE_ILLFUN);
    first = text->ptr[0];
    str_tmp_free(text);
    return repeated(n, first);
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
#pragma aux B_STRI "B$STRI"
#pragma aux B_STRS "B$STRS"
#pragma aux B_FLEN "B$FLEN"
#pragma aux B_SCPF "B$SCPF"
#pragma aux B_SCPY "B$SCPY"
