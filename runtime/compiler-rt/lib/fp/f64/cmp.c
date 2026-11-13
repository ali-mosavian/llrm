/* __cmpdf2, __eqdf2, __nedf2, __ltdf2, __ledf2, __gedf2, __gtdf2. With SF_CMP_SPLIT each is built alone from cmp_<entry>.c (SF_CMP_ONLY_<ENTRY>): __cmpdf2 holds the ordering core, eq, ne, lt and le are aliases of it (one OMF public name each), gt an alias of ge, which calls it. */
#include "f64.h"
#include "../cmp.inc"

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. The core returns 1; ge and gt are the negated reversed compare (unordered stays unordered, ordered flips twice). */
#if defined(SF_CMP_SPLIT) || defined(SF_CMP_OUTLINE) /* OUTLINE: whole objects, no alias; eq/ne/lt/le tail-jump to the cmp entry, ge/gt call it (m32 f64 -O2: seven inlined copies were 1026 B) */
int SF_NAME(cmpdf2)(sf_double a, sf_double b);
#define SF_CMP_FWD(a, b) SF_NAME(cmpdf2)(a, b)
#if defined(__OPTIMIZE_SIZE__) || defined(SF_CMP_NOALIAS) /* ge is a call of the core too: its own inline copy (702 B) is the linker's second copy */
#define SF_CMP_REV(a, b) -SF_NAME(cmpdf2)(b, a)
#else
#ifdef SF_CMP_BRANCHY /* the forward core with -1 for unordered is ge's contract: no negated reversed call */
#define SF_CMP_REV(a, b) sf_cmp_un_64(d2b(a), d2b(b), -1)
#else
#define SF_CMP_REV(a, b) -sf_cmp_bits_64(d2b(b), d2b(a))
#endif
#endif
#else
#define SF_CMP_FWD(a, b) sf_cmp_bits_64(d2b(a), d2b(b))
#ifdef SF_CMP_BRANCHY /* the forward core with -1 for unordered is ge's contract: no negated reversed call */
#define SF_CMP_REV(a, b) sf_cmp_un_64(d2b(a), d2b(b), -1)
#else
#define SF_CMP_REV(a, b) -sf_cmp_bits_64(d2b(b), d2b(a))
#endif
#endif
#ifdef SF_CMP_NOALIAS /* a target with no alias: each split entry is an object of its own that jumps to the core's (eq/ne/lt/le) or to ge (gt) */
int SF_NAME(gedf2)(sf_double a, sf_double b);
#define SF_CMP_GT(a, b) SF_NAME(gedf2)(a, b)
#elif defined(SF_CMP_OUTLINE) /* gt is a jump to ge, which carries the one reversed copy */
#define SF_CMP_GT(a, b) SF_NAME(gedf2)(a, b)
#else
#define SF_CMP_GT(a, b) SF_CMP_REV(a, b)
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_CMP)
SF_CMP_CORE int SF_NAME(cmpdf2)(sf_double a, sf_double b) { return sf_cmp_bits_64(d2b(a), d2b(b)); }
#if defined(SF_CMP_SPLIT) && !defined(SF_CMP_NOALIAS)
int SF_NAME(eqdf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(cmpdf2));
int SF_NAME(nedf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(cmpdf2));
int SF_NAME(ltdf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(cmpdf2));
int SF_NAME(ledf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(cmpdf2));
#endif
#endif
#if (!defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)) || (defined(SF_CMP_NOALIAS) && defined(SF_CMP_ONLY_EQ))
int SF_NAME(eqdf2)(sf_double a, sf_double b) { return SF_CMP_FWD(a, b); }
#endif
#if (!defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)) || (defined(SF_CMP_NOALIAS) && defined(SF_CMP_ONLY_NE))
int SF_NAME(nedf2)(sf_double a, sf_double b) { return SF_CMP_FWD(a, b); }
#endif
#if (!defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)) || (defined(SF_CMP_NOALIAS) && defined(SF_CMP_ONLY_LT))
int SF_NAME(ltdf2)(sf_double a, sf_double b) { return SF_CMP_FWD(a, b); }
#endif
#if (!defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)) || (defined(SF_CMP_NOALIAS) && defined(SF_CMP_ONLY_LE))
int SF_NAME(ledf2)(sf_double a, sf_double b) { return SF_CMP_FWD(a, b); }
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_GE)
SF_CMP_CORE int SF_NAME(gedf2)(sf_double a, sf_double b) { return SF_CMP_REV(a, b); }
#if defined(SF_CMP_SPLIT) && !defined(SF_CMP_NOALIAS)
int SF_NAME(gtdf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(gedf2));
#endif
#endif
#if (!defined(SF_CMP_SPLIT) && !defined(SF_CMP_ALIAS_ENTRIES)) || (defined(SF_CMP_NOALIAS) && defined(SF_CMP_ONLY_GT))
int SF_NAME(gtdf2)(sf_double a, sf_double b) { return SF_CMP_GT(a, b); }
#endif

#ifdef SF_CMP_ALIAS_ENTRIES /* one object, aliases for the entries that are the core's or ge's own result (wasm: a name costs no bytes, a wrapper 12) */
int SF_NAME(eqdf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(cmpdf2));
int SF_NAME(nedf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(cmpdf2));
int SF_NAME(ltdf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(cmpdf2));
int SF_NAME(ledf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(cmpdf2));
int SF_NAME(gtdf2)(sf_double a, sf_double b) SF_CMP_ALIAS(SF_NAME(gedf2));
#endif

#include "../core_end.inc"
