/* String assignment (QB rt/stcore.asm). */
#include "nhstutil.h"

/* B$SASS: the destination takes the source's string.  A temporary source gives
   its up; any other is copied first, so the destination always owns what it
   points at. */
void B_SASS(SD *source, SD *destination)
{
    if (!str_is_tmp(source))
        source = str_tmp_copy(source, 0, source->len);
    str_adopt(destination, source);
}

/* B$SCAT: the two strings joined in a temporary; a temporary operand is freed.
   */
SD *B_SCAT(SD *left, SD *right)
{
    long length = (long)left->len + right->len;
    char *data;
    SD *joined;

    if (length > 32767)
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
