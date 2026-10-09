/* Text to a number or a string (QB rt/fin.asm B$FIN, B$STRSCAN).

   Both read one item from a NUL-terminated stream and return the character that
   ends it (a comma, the NUL, or whatever else follows), with the cursor after
   that character. */
#ifndef QB_FIN_H
#define QB_FIN_H

#include "qb.h"

typedef union FinValue {
    int integer;
    long long_integer;
    float single;
    double real;
} FinValue;

/* A number of type VT_I2, VT_I4, VT_R4 or VT_R8.  Text with no number is 0, as
   in QB, and the caller finds the stray character in the delimiter. Overflow,
   and a type character the number does not fit, are errors. */
char fin_number(const char **cursor, byte type, FinValue *value);

/* A string item: quoted, up to the closing quote, or bare, up to a comma with
   the blanks before it dropped.  *start and *length are the string. */
char fin_string(const char **cursor, const char **start, unsigned *length);

#endif
