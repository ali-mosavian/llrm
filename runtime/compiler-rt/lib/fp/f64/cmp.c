/* __cmpdf2, __eqdf2, __nedf2, __ltdf2, __ledf2, __gedf2, __gtdf2. With SF_CMP_SPLIT each is built alone from cmp_<entry>.c (SF_CMP_ONLY_<ENTRY>): __cmpdf2 holds the ordering core, the others jump or call to it. */
#include "f64.h"
#include "../cmp.inc"

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. The core returns 1; ge and gt are the negated reversed compare (unordered stays unordered, ordered flips twice). */
#ifdef SF_CMP_SPLIT
int SF_NAME(cmpdf2)(sf_double a, sf_double b);
#define SF_CMP_FWD(a, b) SF_NAME(cmpdf2)(a, b)
#define SF_CMP_REV(a, b) -SF_NAME(cmpdf2)(b, a)
#else
#define SF_CMP_FWD(a, b) sf_cmp_bits_64(d2b(a), d2b(b))
#define SF_CMP_REV(a, b) -sf_cmp_bits_64(d2b(b), d2b(a))
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_CMP)
int SF_NAME(cmpdf2)(sf_double a, sf_double b) { return sf_cmp_bits_64(d2b(a), d2b(b)); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_EQ)
int SF_NAME(eqdf2)(sf_double a, sf_double b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_NE)
int SF_NAME(nedf2)(sf_double a, sf_double b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_LT)
int SF_NAME(ltdf2)(sf_double a, sf_double b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_LE)
int SF_NAME(ledf2)(sf_double a, sf_double b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GE)
int SF_NAME(gedf2)(sf_double a, sf_double b) { return SF_CMP_REV(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GT)
int SF_NAME(gtdf2)(sf_double a, sf_double b) { return SF_CMP_REV(a, b); }
#endif

#include "../core_end.inc"
