/* Binary to decimal digits (the math pack's $i8_output, BCOM45 emfout). */
#ifndef QB_I8OUT_H
#define QB_I8OUT_H

#include "qb.h"

enum { MAX_DIGITS = 16 };

/* A number as its sign, its leading digits and the exponent of ten that
   puts the decimal point before the first: value = 0.d1d2... * 10^exponent. */
typedef struct Decimal {
    char sign;                 /* ' ' or '-' */
    char text[MAX_DIGITS];     /* no trailing zeros */
    word count;
    int exponent;
} Decimal;

/* False for an infinity or NaN. */
int i8_output(double value, Decimal *out);

#endif
