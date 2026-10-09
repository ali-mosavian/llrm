/* STR$ of a SINGLE or DOUBLE (QB rt/stringfp.asm), apart from string.c so that
   the digit conversion is linked only by a program that uses it. */
#include "fout.h"
#include "nhstutil.h"

static SD *real_text(double v, int is_double)
{
    char text[FOUT_MAX];
    unsigned length = fout_real(v, is_double, text);
    char *data;
    SD *result = str_tmp(length, &data);

    copy_bytes(data, text, length);
    return result;
}

SD *B_STR4(float v)
{
    return real_text(v, 0);
}

SD *B_STR8(double v)
{
    return real_text(v, 1);
}
#pragma aux B_STR4 "B$STR4"
#pragma aux B_STR8 "B$STR8"
