/* Force-included ahead of every source: the runtime a module links against
   is Borland's, cdecl, while Open Watcom's headers declare their own with
   __watcall. One definition moves every runtime declaration to the target's. */
#define __watcall __cdecl
