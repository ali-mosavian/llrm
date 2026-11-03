/* __cmpdf2, __eqdf2, __nedf2, __ltdf2, __ledf2, __gedf2, __gtdf2. */
#include "f64.h"
#include "../cmp.inc"

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt; see f32/cmp.c. */
int SF_NAME(cmpdf2)(sf_double a, sf_double b) { return cmp_bits_64(d2b(a), d2b(b)); }
int SF_NAME(eqdf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(nedf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(ltdf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(ledf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(gedf2)(sf_double a, sf_double b) { return -cmp_bits_64(d2b(b), d2b(a)); }
int SF_NAME(gtdf2)(sf_double a, sf_double b) { return SF_NAME(gedf2)(a, b); }

#include "../core_end.inc"
