/* STR$ and string deletion (QB rt/string.asm, rt/stringfp.asm). */
#include "fout.h"
#include "nhstutil.h"

/* B$STR_COMMON: a number's text, in a temporary. */
static SD *temporary_of(const char *text, word length)
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

static SD *real_text(double v, int is_double)
{
    char text[FOUT_MAX];

    return temporary_of(text, fout_real(v, is_double, text));
}

SD *B_STI2(int v)
{
    return integer_text(v);
}

SD *B_STI4(long v)
{
    return integer_text(v);
}

SD *B_STR4(float v)
{
    return real_text(v, 0);
}

SD *B_STR8(double v)
{
    return real_text(v, 1);
}

/* B$STDL: deallocate a string, leaving its descriptor empty. */
void B_STDL(SD *sd)
{
    str_release(sd);
    sd->len = 0;
}
#pragma aux B_STI2 "B$STI2"
#pragma aux B_STI4 "B$STI4"
#pragma aux B_STR4 "B$STR4"
#pragma aux B_STR8 "B$STR8"
#pragma aux B_STDL "B$STDL"
