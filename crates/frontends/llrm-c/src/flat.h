/* Force-included ahead of every source of the flat 32-bit target: one address space,
   so `near` and `far` name nothing. `__huge`, `__based` and `__segment` stay what
   Open Watcom makes of them, which llrm-c refuses. */
#define __near
#define __far
#define near
#define far
/* The one Open Watcom keyword Borland's headers use as a plain name: dos.h's parameters (`peek( unsigned __segment, ...)`),
   as in borland.h. Borland's own sources never mean Open Watcom's segment type, which a flat target has no use for. */
#define __segment __borland_segment
