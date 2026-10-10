/* Binary to decimal digits (the math pack's $i8_output, BCOM45 emfout). */
#ifndef QB_I8OUT_H
#define QB_I8OUT_H

#include "qb.h"

enum { MAX_DIGITS = 16 };

/* A number as its sign, its leading digits and the exponent of ten that puts
   the decimal point before the first: value = 0.d1d2... * 10^exponent. */
typedef struct Decimal {
    char sign;                 /* ' ' or '-' */
    char text[MAX_DIGITS];     /* no trailing zeros */
    unsigned count;
    int exponent;
} Decimal;

/* x * 10^k in extended precision, the math pack's own way (it scales both the
   digits QB prints and the numbers it reads). */
long double i8_scale(long double x, int k);

/* The digits of `value`; an infinity or NaN comes out as the text 1#INF, 1#NAN
   or 1#IND, as in QB. */
void i8_output(double value, Decimal *out);

#endif
