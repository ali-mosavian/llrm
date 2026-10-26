/* Borland C's keywords on clang's i8086 target, force-included for x86_16-dos. `far`, `huge` and `near` are address
   spaces 1, 3 and 0; a function is far in the medium model, which the target drops from `void`. `pascal` is clang's
   attribute, numbered for llrm. */
#define __BORLANDC__ 0x0410
#define __TURBOC__ 0x0410
#define __MEDIUM__ 1
#define __MSDOS__ 1
#define near
#define __near
#define _near
#define far __attribute__((address_space(1)))
#define __far __attribute__((address_space(1)))
#define _far __attribute__((address_space(1)))
#define huge __attribute__((address_space(3)))
#define __huge __attribute__((address_space(3)))
#define _huge __attribute__((address_space(3)))
#define cdecl
#define _cdecl
#define __cdecl
#define _Cdecl
#define pascal __attribute__((pascal))
#define _pascal __attribute__((pascal))
#define __pascal __attribute__((pascal))
#undef __STDC__
