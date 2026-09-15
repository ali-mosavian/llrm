/* Force-included ahead of every source: the runtime a module links against
   is Borland's, cdecl, while Open Watcom's headers declare their own with
   __watcall. One definition moves every runtime declaration to the target's. */
#define __watcall __cdecl
/* The one Open Watcom keyword Borland's headers use as a plain name: dos.h's
   parameters. Borland's own sources never mean Open Watcom's segment type. */
#define __segment __borland_segment
/* bcc without -A: its headers show dos.h's FP_OFF and the rest only then. */
#undef __STDC__
