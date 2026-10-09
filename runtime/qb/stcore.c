/* String assignment (QB rt/stcore.asm). */
#include "nhstutil.h"

/* B$SASS: the destination takes the source's string.  A temporary source gives its up; any other is
   copied first, so the destination always owns what it points at. */
void B_SASS(
    SD *source,
    SD *destination)
{
    if (!str_is_tmp(source))
        source = str_tmp_copy(source, 0, source->len);
    str_adopt(destination, source);
}
#pragma aux B_SASS "B$SASS"
