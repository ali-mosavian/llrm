/* Fixed-length strings (STRING * n in a TYPE or a variable): assigning one and
   reading one as a variable-length string. */
#include "nhstutil.h"
#include "qb.h"

/* B$ASSN: the `source_length` bytes at `source` into the `length` bytes at
   `target`: cut to fit, or padded with blanks. */
void B_ASSN(int source_length, int length, qb_data_ptr source, qb_data_ptr target)
{
    char *to = QB_NEAR_OF(target);
    const char *from = QB_NEAR_OF(source);
    int kept = source_length < length ? source_length : length;

    copy_bytes(to, from, kept);
    for (to += kept; kept < length; kept++)
        *to++ = ' ';
}

/* B$LDFS: the `length` bytes at `field` as a temporary string. */
SD *B_LDFS(int length, qb_data_ptr field)
{
    char *data;
    SD *result = str_tmp(length, &data);

    copy_bytes(data, QB_NEAR_OF(field), length);
    return result;
}
#pragma aux B_ASSN "B$ASSN"
#pragma aux B_LDFS "B$LDFS"
