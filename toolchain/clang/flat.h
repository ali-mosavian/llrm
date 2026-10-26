/* Force-included ahead of every source of the flat 32-bit target: one address space, so `near` and `far` name
   nothing (the Watcom front end's flat.h). */
#define __near
#define __far
#define near
#define far
