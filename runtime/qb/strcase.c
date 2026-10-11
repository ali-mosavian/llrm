/* UCASE$ and LCASE$ (QB rt/strfcn.asm). */
#include "nhstutil.h"

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

#pragma aux B_UCAS "B$UCAS"
#pragma aux B_LCAS "B$LCAS"
