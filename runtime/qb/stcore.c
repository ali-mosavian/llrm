/* String assignment (QB rt/stcore.asm B$SASS). */
#include "nhstutil.h"

/* A temporary source gives up its data to the destination; any other source is copied into a
   temporary first, so the destination always owns what it points at. */
void QB B_SASS(word src, word dst)
{
    SD *s = (SD *)src, *d = (SD *)dst;
    word len, ptr;

    if (!str_is_tmp(s))
        s = str_tmp_sub(s, 0, s->len);
    str_free_sd(d);
    len = d->len = s->len;
    ptr = d->ptr = s->ptr;
    if (len) {
        W(ptr - 2) = dst;
        str_tmp_release(s);
    }
}
#pragma aux B_SASS "B$SASS"
