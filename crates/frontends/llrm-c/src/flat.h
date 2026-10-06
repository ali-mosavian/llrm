/* Force-included ahead of every source of the flat 32-bit target: one address space,
   so `near` and `far` name nothing. `__huge`, `__based` and `__segment` stay what
   Open Watcom makes of them, which llrm-c refuses. */
#define __near
#define __far
#define near
#define far
