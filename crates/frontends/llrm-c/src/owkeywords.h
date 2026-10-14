/* Open Watcom keywords that the Borland sources and headers use as plain names, renamed once for every target: dos.h's parameters
   (`peek( unsigned __segment, unsigned __offset )`). Borland's own sources never mean Open Watcom's segment type. Included by
   borland.h and flat.h; the next such keyword is added here. */
#define __segment __borland_segment

/* gcc's `__attribute__((noinline, unused))`: Open Watcom reads the list as `__declspec( ... )` (patches/ctype.c.patch), which
   takes the attributes that state a fact or change no meaning and refuses the rest. */
#define __attribute__(list) __declspec list
