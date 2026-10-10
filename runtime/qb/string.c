/* STR$ of an integer and string deletion (QB rt/string.asm). */
#include "fout.h"
#include "nhstutil.h"

/* B$STR_COMMON: a number's text, in a temporary. */
static SD *temporary_of(const char *text, unsigned length)
{
    char *data;
    SD *result = str_tmp(length, &data);

    copy_bytes(data, text, length);
    return result;
}

static SD *integer_text(long v)
{
    char text[FOUT_MAX];

    return temporary_of(text, fout_i4(v, text));
}

SD *B_STI2(short v)
{
    return integer_text(v);
}

SD *B_STI4(long v)
{
    return integer_text(v);
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
