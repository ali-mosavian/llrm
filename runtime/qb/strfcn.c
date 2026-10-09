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
/* LEFT$ and RIGHT$: `len` bytes from `from` of the argument, fewer where it
   ends, in a temporary; an empty result frees the argument. */
static SD *slice(SD *sd, unsigned from, int len)
{
    if (len < 0)
        qb_error(BE_ILLFUN);
    if (len == 0) {
        str_tmp_free(sd);
        return &str_nul;
    }
    if ((unsigned)len > sd->len - from)
        len = sd->len - from;
    return str_tmp_copy(sd, from, len);
}

SD *B_LEFT(SD *sd, int len)
{
    return slice(sd, 0, len);
}

SD *B_RGHT(SD *sd, int len)
{
    unsigned skip = len > 0 && (unsigned)len < sd->len ? sd->len - len : 0;

    return slice(sd, skip, len);
}

/* B$FMID: MID$(s, start, len), start counting from 1. */
SD *B_FMID(SD *sd, int start, int len)
{
    if (len < 0 || start < 1)
        qb_error(BE_ILLFUN);
    if ((unsigned)start > sd->len) {
        str_tmp_free(sd);
        return &str_nul;
    }
    return slice(sd, start - 1, len);
}

/* B$FCHR: CHR$(code). */
SD *B_FCHR(int code)
{
    char *data;
    SD *result;

    if (code < 0 || code > 255)
        qb_error(BE_ILLFUN);
    result = str_tmp(1, &data);
    *data = (char)code;
    return result;
}

/* B$SCMP: -1, 0 or 1 as the first string sorts before, with or after the
   second; both are consumed if temporary. */
int B_SCMP(SD *first, SD *second)
{
    unsigned common = first->len < second->len ? first->len : second->len;
    unsigned at;
    int order = 0;

    for (at = 0; at < common && order == 0; at++)
        order = first->ptr[at] - second->ptr[at];
    if (order == 0)
        order = first->len == second->len ? 0
              : first->len < second->len ? -1 : 1;
    str_tmp_free(first);
    str_tmp_free(second);
    return order < 0 ? -1 : order > 0;
}

#pragma aux B_LTRM "B$LTRM"
#pragma aux B_RTRM "B$RTRM"
#pragma aux B_SPAC "B$SPAC"
#pragma aux B_STRI "B$STRI"
#pragma aux B_STRS "B$STRS"
#pragma aux B_FLEN "B$FLEN"
#pragma aux B_SCPF "B$SCPF"
#pragma aux B_UCAS "B$UCAS"
#pragma aux B_LCAS "B$LCAS"
#pragma aux B_SCPY "B$SCPY"
#pragma aux B_LEFT "B$LEFT"
#pragma aux B_RGHT "B$RGHT"
#pragma aux B_FMID "B$FMID"
#pragma aux B_FCHR "B$FCHR"
#pragma aux B_SCMP "B$SCMP"
