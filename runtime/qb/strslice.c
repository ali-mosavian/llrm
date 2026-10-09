/* LEFT$, RIGHT$, MID$, CHR$ and string comparison (QB rt/strfcn.asm,
   rt/stcore.asm). */
#include "nhstutil.h"

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

#pragma aux B_LEFT "B$LEFT"
#pragma aux B_RGHT "B$RGHT"
#pragma aux B_FMID "B$FMID"
#pragma aux B_FCHR "B$FCHR"
#pragma aux B_SCMP "B$SCMP"
