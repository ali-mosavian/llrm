/* __cmpsf2 alone (SF_CMP_SPLIT only). */
#ifdef SF_CMP_SPLIT
#define SF_CMP_ONLY_CMP
#include "cmp.c"
#undef SF_CMP_ONLY_CMP
#endif
