/* String functions (QB rt/strfcn.asm).  Every result is a temporary: the argument is never changed. */
#include "nhstutil.h"

enum { TR_LEFT = 1, TR_RIGHT = 2 };

/* TRIM: the argument itself when it is empty, else a temporary holding the part without the blanks. */
static SD *trim(word sd, byte sides)
{
    SD *s = (SD *)sd;
    word first = 0, last = s->len;
    byte *p = (byte *)s->ptr;

    if (s->len == 0)
        return s;
    if (sides & TR_LEFT)
        while (first < last && p[first] == ' ')
            first++;
    if (sides & TR_RIGHT)
        while (last > first && p[last - 1] == ' ')
            last--;
    return str_tmp_sub(s, first, last - first);
}

SD *QB B_LTRM(word sd) { return trim(sd, TR_LEFT); }
SD *QB B_RTRM(word sd) { return trim(sd, TR_RIGHT); }

/* B$SPAC: SPACE$(n). */
SD *QB B_SPAC(int n)
{
    word data, i;
    SD *t;

    if (n < 0)
        qb_error(BE_ILLFUN);
    t = str_tmp(n, &data);
    for (i = 0; i < (word)n; i++)
        B(data + i) = ' ';
    return t;
}
#pragma aux B_LTRM "B$LTRM"
#pragma aux B_RTRM "B$RTRM"
#pragma aux B_SPAC "B$SPAC"

/* B$FLEN: LEN of a string; a temporary is consumed. */
int QB B_FLEN(word sd)
{
    int len = ((SD *)sd)->len;

    str_tmp_free((SD *)sd);
    return len;
}
#pragma aux B_FLEN "B$FLEN"
