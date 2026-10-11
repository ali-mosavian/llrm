/* QB's INITIALIZER (inc/rmacros.inc): the module runs XI_FN at startup, and
   only if it is linked. Define XI_FN, then include this once.  The pads put
   XIB, XI and XIE in that order in every module that has one, so whichever the
   linker meets first fixes the order. */
#include "platform.h"
#define XI_JOIN2(a, b) a##b
#define XI_JOIN(a, b) XI_JOIN2(a, b)
void XI_FN(void);
#pragma data_seg("XIB", "DATA")
const qb_init_fn XI_JOIN(xib_, XI_FN) = 0;
#pragma data_seg("XI", "DATA")
const qb_init_fn XI_JOIN(xi_, XI_FN) = XI_FN;
#pragma data_seg("XIE", "DATA")
const qb_init_fn XI_JOIN(xie_, XI_FN) = 0;
#pragma data_seg()
