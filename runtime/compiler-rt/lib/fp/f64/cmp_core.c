/* The ordering core every entry calls (SF_CMP_SPLIT only). */
#ifdef SF_CMP_SPLIT
#define SF_CMP_CORE
#include "cmp.c"
#undef SF_CMP_CORE
#endif
