/* String assignment (QB rt/stcore.asm). */
#include "nhstutil.h"

/* The destination takes the source's string.  A temporary source gives its up;
   any other is copied first, so the destination always owns what it points at.
   */
void str_assign(SD *source, SD *destination)
{
    if (str_is_tmp(source)) {
        str_adopt(destination, source);
        return;
    }
    /* Copied straight into the destination, which gives up its own string first: a temporary between
       would be one more allocation and copy. */
    if (source != destination) {
        uword len = source->len;

        str_release(destination);
        if (len == 0) {
            destination->len = 0;
            destination->ptr = str_nul.ptr;
        } else {
            char *data = str_alloc(destination, len);

            copy_bytes(data, source->ptr, len);
        }
    }
}

/* B$SASS: LET a$ = b$ */
void B_SASS(SD *source, SD *destination)
{
    str_assign(source, destination);
}

/* B$SCAT: the two strings joined in a temporary; a temporary operand is freed.
   */
SD *B_SCAT(SD *left, SD *right)
{
    long length = (long)left->len + right->len;
    char *data;
    SD *joined;

    if (length > QB_STRING_LIMIT)
        qb_error(BE_ILLFUN);
    joined = str_tmp(length, &data);
    copy_bytes(data, left->ptr, left->len);
    copy_bytes(data + left->len, right->ptr, right->len);
    str_tmp_free(left);
    str_tmp_free(right);
    return joined;
}
#pragma aux B_SASS "B$SASS"
#pragma aux B_SCAT "B$SCAT"
