/* Fixed-length strings (STRING * n in a TYPE or a variable): assigning one and
   reading one as a variable-length string. */
#include "fixstr.h"
#include "nhstutil.h"
#include "qb.h"

#pragma aux B_ASSN "B$ASSN"
#pragma aux B_LSET "B$LSET"
#pragma aux B_RSET "B$RSET"
#pragma aux B_LDFS "B$LDFS"

/* B$ASSN: the `source_length` bytes at `source` into the `length` bytes at
   `target`: cut to fit, or padded with blanks. */
void B_ASSN(qb_data_ptr source, int source_length, qb_data_ptr target, int length)
{
    fixed_assign(QB_NEAR_OF(target), QB_NEAR_OF(source), source_length, length);
}

/* B$LDFS: the `length` bytes at `field` as a temporary string. */
SD *B_LDFS(qb_data_ptr field, int length)
{
    char *data;
    SD *result = str_tmp(length, &data);

    copy_bytes(data, QB_NEAR_OF(field), length);
    return result;
}

/* LSET and RSET: `source` into the field at `destination` (`length` bytes, or the string whose descriptor it is when
   `length` is 0), the shorter padded with blanks on the right (LSET) or the left (RSET), the longer cut to fit on the
   right. A temporary source is freed. */
static void justify(SD *source, qb_data_ptr destination, int length, int right)
{
    char *to = QB_NEAR_OF(destination);
    int kept, pad;

    if (length == 0) {
        SD *target = (SD *)to;

        to = target->ptr;
        length = target->len;
    }
    kept = source->len < length ? source->len : length;
    pad = length - kept;
    if (right) {
        char *at = to;
        int count = pad;

        for (; count; count--)
            *at++ = ' ';
        copy_bytes(to + pad, source->ptr, kept);
    } else {
        copy_bytes(to, source->ptr, kept);
        for (to += kept; pad; pad--)
            *to++ = ' ';
    }
    str_tmp_free(source);
}

void B_LSET(SD *source, qb_data_ptr destination, int length)
{
    justify(source, destination, length, 0);
}

void B_RSET(SD *source, qb_data_ptr destination, int length)
{
    justify(source, destination, length, 1);
}
