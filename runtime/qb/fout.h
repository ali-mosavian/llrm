/* Number to text, the way STR$ and PRINT write it (QB rt/ifout.asm, rt/fout.asm). */
#ifndef QB_FOUT_H
#define QB_FOUT_H

#include "qb.h"

enum { FOUT_MAX = 34 };

/* Writes the text of the INTEGER or LONG `v` to `out`: a sign column (' ' or '-'), then the digits.
   Returns its length. */
word fout_i4(long v, char *out);

#endif
