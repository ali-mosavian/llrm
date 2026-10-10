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

/* B$SMID: MID$(target, start, max) = source, overwriting in place: a fixed
   string has `width` bytes at `target`, else `target` is the descriptor. */
void B_SMID(qb_data_ptr target, int width, SD *source, int max, int start)
{
    char *data;
    unsigned room, count;

    if (start < 1 || max < 0)
        qb_error(BE_ILLFUN);
    if (width) {
        data = (char *)QB_NEAR_OF(target);
        room = width;
    } else {
        SD *sd = QB_NEAR_OF(target);

        data = sd->ptr;
        room = sd->len;
    }
    if ((unsigned)start > room)
        qb_error(BE_ILLFUN);
    data += start - 1;
    count = room - (start - 1);
    if (count > (unsigned)max)
        count = max;
    if (count > source->len)
        count = source->len;
    /* the source may be the target: move forwards byte by byte */
    copy_bytes(data, source->ptr, count);
    str_tmp_free(source);
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
#pragma aux B_SMID "B$SMID"
#pragma aux B_FCHR "B$FCHR"
#pragma aux B_SCMP "B$SCMP"

/* INSTR: where the match first is in the source from `start` (1 or more), 0 if
   it is not.  An empty match is at `start` unless that is past the end; an
   empty source has none. */
static int instr(unsigned start, SD *source, SD *match)
{
    unsigned at, i;
    int found = 0;

    if (source->len != 0 && start <= source->len) {
        found = start;
        if (match->len != 0) {
            found = 0;
            for (at = start - 1; at + match->len <= source->len && !found;
                 at++) {
                for (i = 0; i < match->len
                            && source->ptr[at + i] == match->ptr[i]; i++)
                    ;
                if (i == match->len)
                    found = at + 1;
            }
        }
    }
    str_tmp_free(source);
    str_tmp_free(match);
    return found;
}

int B_INS2(SD *source, SD *match)
{
    return instr(1, source, match);
}

int B_INS3(int start, SD *source, SD *match)
{
    if (start <= 0)
        qb_error(BE_ILLFUN);
    return instr(start, source, match);
}
#pragma aux B_INS2 "B$INS2"
#pragma aux B_INS3 "B$INS3"
