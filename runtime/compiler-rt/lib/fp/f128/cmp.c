/* __cmptf2, __eqtf2, __netf2, __lttf2, __letf2, __getf2, __gttf2. With SF_CMP_SPLIT each is built alone from cmp_<entry>.c (SF_CMP_ONLY_<ENTRY>): __cmptf2 holds the ordering core, the others jump or call to it. */
#include "f128.h"
#include "../cmp.inc"

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. The core returns 1; ge and gt are the negated reversed compare (unordered stays unordered, ordered flips twice). */
#ifdef SF_CMP_SPLIT
int SF_NAME(cmptf2)(sf_quad a, sf_quad b);
#define SF_CMP_FWD(a, b) SF_NAME(cmptf2)(a, b)
#define SF_CMP_REV(a, b) -SF_NAME(cmptf2)(b, a)
#else
#define SF_CMP_FWD(a, b) sf_cmp_bits_128(q2b(a), q2b(b))
#define SF_CMP_REV(a, b) -sf_cmp_bits_128(q2b(b), q2b(a))
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_CMP)
int SF_NAME(cmptf2)(sf_quad a, sf_quad b) { return sf_cmp_bits_128(q2b(a), q2b(b)); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_EQ)
int SF_NAME(eqtf2)(sf_quad a, sf_quad b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_NE)
int SF_NAME(netf2)(sf_quad a, sf_quad b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_LT)
int SF_NAME(lttf2)(sf_quad a, sf_quad b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_LE)
int SF_NAME(letf2)(sf_quad a, sf_quad b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GE)
int SF_NAME(getf2)(sf_quad a, sf_quad b) { return SF_CMP_REV(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GT)
int SF_NAME(gttf2)(sf_quad a, sf_quad b) { return SF_CMP_REV(a, b); }
#endif

#include "../core_end.inc"
