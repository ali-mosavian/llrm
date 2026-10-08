/* Force-included ahead of every source: the runtime a module links against
   is Borland's, cdecl, while Open Watcom's headers declare their own with
   __watcall. One definition moves every runtime declaration to the target's. */
#ifndef LLRM_WATCOM_ABI
#define __watcall __cdecl
#endif
/* Source conditionals must see the compiler ABI we emit. qcport uses this
   to retain explicit far data pointers in its Borland medium-model build. */
#ifndef __BORLANDC__
#define __BORLANDC__ 0x0410
#endif
#include "owkeywords.h"
/* bcc without -A: its headers show dos.h's FP_OFF and the rest only then. */
#undef __STDC__
