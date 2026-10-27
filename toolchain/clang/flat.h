/* Force-included ahead of every source of the flat 32-bit target: one address space, so `near`, `far` and `huge` name
   nothing (the Watcom front end's flat.h). */
#define __near
#define __far
#define near
#define far
#define __huge
#define huge
