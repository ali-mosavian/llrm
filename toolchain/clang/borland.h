/* Borland C's keywords on clang's i8086 target, force-included for x86_16-dos. `far`, `huge` and `near` map to the fork's
   `__far__`, `__huge__` and `__near__` (address spaces 1, 3 and 0); a function is far in the medium model, which the target drops from `void`. `pascal` is clang's
   attribute, numbered for llrm. */
#define __BORLANDC__ 0x0410
#define __TURBOC__ 0x0410
#define __MSDOS__ 1
#define near __near__
#define __near __near__
#define _near __near__
#define far __far__
#define __far __far__
#define _far __far__
#define huge __huge__
#define __huge __huge__
#define _huge __huge__
#define cdecl
#define _cdecl
#define __cdecl
#define _Cdecl
#define pascal __attribute__((pascal))
#define _pascal __attribute__((pascal))
#define __pascal __attribute__((pascal))
#define interrupt
