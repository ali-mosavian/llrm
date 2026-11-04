/* __cmpsf2, __eqsf2, __nesf2, __ltsf2, __lesf2, __gesf2, __gtsf2. With SF_CMP_SPLIT each is built alone from cmp_<entry>.c (SF_CMP_ONLY_<ENTRY>): __cmpsf2 holds the ordering core, eq, ne, lt and le are aliases of it (one OMF public name each), gt an alias of ge, which calls it. */
#include "f32.h"
#include "../cmp.inc"

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. The core returns 1; ge and gt are the negated reversed compare (unordered stays unordered, ordered flips twice). */
#ifdef SF_CMP_SPLIT
int SF_NAME(cmpsf2)(sf_float a, sf_float b);
#define SF_CMP_FWD(a, b) SF_NAME(cmpsf2)(a, b)
#define SF_CMP_REV(a, b) -SF_NAME(cmpsf2)(b, a)
#else
#define SF_CMP_FWD(a, b) sf_cmp_bits(f2b(a), f2b(b))
#define SF_CMP_REV(a, b) -sf_cmp_bits(f2b(b), f2b(a))
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_CMP)
int SF_NAME(cmpsf2)(sf_float a, sf_float b) { return sf_cmp_bits(f2b(a), f2b(b)); }
#ifdef SF_CMP_SPLIT
int SF_NAME(eqsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(nesf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(ltsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(lesf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
#endif
#endif
#if !defined(SF_CMP_SPLIT)
int SF_NAME(eqsf2)(sf_float a, sf_float b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT)
int SF_NAME(nesf2)(sf_float a, sf_float b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT)
int SF_NAME(ltsf2)(sf_float a, sf_float b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT)
int SF_NAME(lesf2)(sf_float a, sf_float b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GE)
int SF_NAME(gesf2)(sf_float a, sf_float b) { return SF_CMP_REV(a, b); }
#ifdef SF_CMP_SPLIT
int SF_NAME(gtsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(gesf2));
#endif
#endif
#if !defined(SF_CMP_SPLIT)
int SF_NAME(gtsf2)(sf_float a, sf_float b) { return SF_CMP_REV(a, b); }
#endif

#include "../core_end.inc"
