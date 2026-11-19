/* __gedf2 alone (SF_CMP_SPLIT only). */
#ifdef SF_CMP_SPLIT
#define SF_CMP_ONLY_GE
#define sf_cmp_bits_64 sf_cmp_bits_ge_64 /* a copy of its own at -O2, apart from the cmp entry's when both are flattened into one file */
#include "cmp.c"
#undef SF_CMP_ONLY_GE
#undef sf_cmp_bits_64
#endif
