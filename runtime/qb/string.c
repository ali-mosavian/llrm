/* STR$ and string deletion (QB rt/string.asm). */
#include "fout.h"
#include "nhstutil.h"

/* B$STR_COMMON: STR$ of a number is a temporary holding its text. */
static SD *text_of(long v)
{
    char text[FOUT_MAX], *data;
    word length = fout_i4(v, text);
    SD *result = str_tmp(length, &data);

    copy_bytes(data, text, length);
    return result;
}

SD *B_STI2(int v)
{
    return text_of(v);
}

SD *B_STI4(long v)
{
    return text_of(v);
}

/* B$STDL: deallocate a string, leaving its descriptor empty. */
void B_STDL(SD *sd)
{
    str_release(sd);
    sd->len = 0;
}
#pragma aux B_STI2 "B$STI2"
#pragma aux B_STI4 "B$STI4"
#pragma aux B_STDL "B$STDL"
