/* __cmpsf2, __eqsf2, __nesf2, __ltsf2, __lesf2, __gesf2, __gtsf2, __unordsf2. */
#include "f32.h"
#include "../cmp.inc"

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt; the core takes it as an argument so each entry point is a plain call. */
int SF_NAME(cmpsf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b), 1); }
int SF_NAME(eqsf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b), 1); }
int SF_NAME(nesf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b), 1); }
int SF_NAME(ltsf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b), 1); }
int SF_NAME(lesf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b), 1); }
int SF_NAME(gesf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b), -1); }
int SF_NAME(gtsf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b), -1); }
int SF_NAME(unordsf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b), 2) == 2; }

#include "../core_end.inc"
