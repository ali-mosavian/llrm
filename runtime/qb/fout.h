/* Number to text, the way STR$ and PRINT write it (QB rt/ifout.asm,
   rt/fout.asm). */
#ifndef QB_FOUT_H
#define QB_FOUT_H

#include "i8out.h"
#include "qb.h"

enum { FOUT_MAX = 34 };

/* Writes the text of the INTEGER or LONG `v` to `out`: a sign column (' ' or
   '-'), then the digits.  Returns its length. */
unsigned fout_i4(long v, char *out);

/* The same for a SINGLE (7 digits) or DOUBLE (16): a sign column, then digits,
   or digits with an exponent after E (D for a double) when the number needs
   more places than that. */
unsigned fout_real(double v, int is_double, char *out);

/* The digits of a SINGLE (7 places) or DOUBLE (16), as PRINT rounds them, and the
   exponent of ten of the last digit: v = d.text * 10^exponent as an integer. */
void fout_digits(double v, int is_double, Decimal *d, int *exponent);

#endif
