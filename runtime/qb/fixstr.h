#ifndef QB_FIXSTR_H
#define QB_FIXSTR_H

#include "qb.h"

/* The `source_length` bytes at `from` into the `length` bytes at `to`: cut to fit, or padded with blanks. In the
   header, not a routine, so READ into a fixed string links no more than it reads. */
static void fixed_assign(char *to, const char *from, int source_length, int length)
{
    int kept = source_length < length ? source_length : length;

    copy_bytes(to, from, kept);
    for (to += kept; kept < length; kept++)
        *to++ = ' ';
}

#endif
