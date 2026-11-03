/* __cmpsf2, __eqsf2, __nesf2, __ltsf2, __lesf2, __gesf2, __gtsf2. With SF_CMP_SPLIT each is built alone from cmp_<entry>.c (SF_CMP_ONLY_<ENTRY>) over the one external core of cmp_core.c (SF_CMP_CORE). */
#include "f32.h"
#include "../cmp.inc"

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. The core returns 1; ge and gt are the negated reversed compare (unordered stays unordered, ordered flips twice). */
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_CMP)
int SF_NAME(cmpsf2)(sf_float a, sf_float b) { return sf_cmp_bits(f2b(a), f2b(b)); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_EQ)
int SF_NAME(eqsf2)(sf_float a, sf_float b) { return sf_cmp_bits(f2b(a), f2b(b)); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_NE)
int SF_NAME(nesf2)(sf_float a, sf_float b) { return sf_cmp_bits(f2b(a), f2b(b)); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_LT)
int SF_NAME(ltsf2)(sf_float a, sf_float b) { return sf_cmp_bits(f2b(a), f2b(b)); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_LE)
int SF_NAME(lesf2)(sf_float a, sf_float b) { return sf_cmp_bits(f2b(a), f2b(b)); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GE)
int SF_NAME(gesf2)(sf_float a, sf_float b) { return -sf_cmp_bits(f2b(b), f2b(a)); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GT)
int SF_NAME(gtsf2)(sf_float a, sf_float b) { return -sf_cmp_bits(f2b(b), f2b(a)); }
#endif

#include "../core_end.inc"
