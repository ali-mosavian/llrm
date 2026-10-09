/* STR$ and string deletion (QB rt/string.asm). */
#include "fout.h"
#include "nhstutil.h"

/* B$STR_COMMON: STR$ of a number is a temporary holding its text. */
static SD *str_of(long v)
{
    byte text[FOUT_MAX];
    word len = fout_i4(v, text), data, n;
    SD *t = str_tmp(len, &data);

    for (n = 0; n < len; n++)
        B(data + n) = text[n];
    return t;
}

SD *QB B_STI2(int v) { return str_of(v); }
SD *QB B_STI4(long v) { return str_of(v); }

/* B$STDL: deallocate a string and leave its descriptor empty. */
void QB B_STDL(word sd)
{
    str_free_sd((SD *)sd);
    ((SD *)sd)->len = 0;
}
#pragma aux B_STI2 "B$STI2"
#pragma aux B_STI4 "B$STI4"
#pragma aux B_STDL "B$STDL"
