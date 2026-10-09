/* QB's INITIALIZER (inc/rmacros.inc): the module runs XI_FN at startup, and
   only if it is linked. Define XI_FN, then include this once.  The pads put
   XIB, XI and XIE in that order in every module that has one, so whichever the
   linker meets first fixes the order. */
#define XI_JOIN2(a, b) a##b
#define XI_JOIN(a, b) XI_JOIN2(a, b)
void XI_FN(void);
#pragma data_seg("XIB", "DATA")
void (__far *const XI_JOIN(xib_, XI_FN))(void) = 0;
#pragma data_seg("XI", "DATA")
void (__far *const XI_JOIN(xi_, XI_FN))(void) = XI_FN;
#pragma data_seg("XIE", "DATA")
void (__far *const XI_JOIN(xie_, XI_FN))(void) = 0;
#pragma data_seg()
