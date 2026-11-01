/* B$SMID: the MID$ statement (QB rt/mid.asm). */
#include "nhstutil.h"

#pragma aux B_SMID "B$SMID"

/* Up to `maximum` bytes of `source` over the string at `destination` from `start` (counting from 1): the `width`
   bytes of a fixed-length string, or the string whose descriptor it is when `width` is 0. The string keeps its length.
   A start outside it or a negative maximum is an illegal function call, the source then left alone; a temporary
   source is freed. The bytes go forward one at a time, for a source that overlaps the destination. */
void B_SMID(qb_data_ptr destination, int width, SD *source, int maximum, int start)
{
    char *to = QB_NEAR_OF(destination);
    unsigned length = (unsigned)width;
    const char *from = source->ptr;
    unsigned count;

    if (start < 1 || maximum < 0)
        qb_error(BE_ILLFUN);
    if (width == 0) {
        SD *target = (SD *)to;

        length = target->len;
        to = target->ptr;
    }
    if ((unsigned)(start - 1) >= length)
        qb_error(BE_ILLFUN);
    to += start - 1;
    count = length - (unsigned)(start - 1);
    if (count > (unsigned)maximum)
        count = (unsigned)maximum;
    if (count > source->len)
        count = source->len;
    for (; count; count--)
        *to++ = *from++;
    str_tmp_free(source);
}
