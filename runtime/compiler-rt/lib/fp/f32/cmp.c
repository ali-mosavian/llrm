/* __cmpsf2, __eqsf2, __nesf2, __ltsf2, __lesf2, __gesf2, __gtsf2. With SF_CMP_SPLIT each is built alone from cmp_<entry>.c (SF_CMP_ONLY_<ENTRY>): __cmpsf2 holds the ordering core, eq, ne, lt and le are aliases of it (one OMF public name each), gt an alias of ge, which calls it. */
#include "f32.h"
#include "../cmp.inc"

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. The core returns 1; ge and gt are the negated reversed compare (unordered stays unordered, ordered flips twice). */
#if defined(SF_CMP_SPLIT) || defined(SF_CMP_OUTLINE) /* OUTLINE: whole objects, no alias; eq/ne/lt/le tail-jump to the cmp entry, ge/gt call it (m32 f64 -O2: seven inlined copies were 1026 B) */
int SF_NAME(cmpsf2)(sf_float a, sf_float b);
#define SF_CMP_FWD(a, b) SF_NAME(cmpsf2)(a, b)
#if defined(__OPTIMIZE_SIZE__)
#define SF_CMP_REV(a, b) -SF_NAME(cmpsf2)(b, a)
#else
#ifdef SF_CMP_BRANCHY /* the forward core with -1 for unordered is ge's contract: no negated reversed call */
#define SF_CMP_REV(a, b) sf_cmp_un(f2b(a), f2b(b), -1)
#else
#define SF_CMP_REV(a, b) -sf_cmp_bits(f2b(b), f2b(a))
#endif
#endif
#else
#define SF_CMP_FWD(a, b) sf_cmp_bits(f2b(a), f2b(b))
#ifdef SF_CMP_BRANCHY /* the forward core with -1 for unordered is ge's contract: no negated reversed call */
#define SF_CMP_REV(a, b) sf_cmp_un(f2b(a), f2b(b), -1)
#else
#define SF_CMP_REV(a, b) -sf_cmp_bits(f2b(b), f2b(a))
#endif
#endif
#ifdef SF_CMP_OUTLINE /* gt is a jump to ge, which carries the one reversed copy */
#define SF_CMP_GT(a, b) SF_NAME(gesf2)(a, b)
#else
#define SF_CMP_GT(a, b) SF_CMP_REV(a, b)
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_CMP)
SF_CMP_CORE int SF_NAME(cmpsf2)(sf_float a, sf_float b) { return sf_cmp_bits(f2b(a), f2b(b)); }
#ifdef SF_CMP_SPLIT
int SF_NAME(eqsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(nesf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(ltsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(lesf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
#endif
#endif
#if !defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)
int SF_NAME(eqsf2)(sf_float a, sf_float b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)
int SF_NAME(nesf2)(sf_float a, sf_float b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)
int SF_NAME(ltsf2)(sf_float a, sf_float b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)
int SF_NAME(lesf2)(sf_float a, sf_float b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GE)
SF_CMP_CORE int SF_NAME(gesf2)(sf_float a, sf_float b) { return SF_CMP_REV(a, b); }
#ifdef SF_CMP_SPLIT
int SF_NAME(gtsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(gesf2));
#endif
#endif
#if !defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)
int SF_NAME(gtsf2)(sf_float a, sf_float b) { return SF_CMP_GT(a, b); }
#endif

#ifdef SF_CMP_ALIAS_ENTRIES /* one object, aliases for the entries that are the core's or ge's own result (wasm: a name costs no bytes, a wrapper 12) */
int SF_NAME(eqsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(nesf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(ltsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(lesf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(cmpsf2));
int SF_NAME(gtsf2)(sf_float a, sf_float b) SF_CMP_ALIAS(SF_NAME(gesf2));
#endif

#include "../core_end.inc"
