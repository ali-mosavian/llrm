/* <stdarg.h> for Borland C++'s headers on clang: Borland's takes `&parmN`, which clang keeps in a local copy, so va_arg read the stack below the arguments. */
#ifndef _BORLAND_STDARG_SHIM
#define _BORLAND_STDARG_SHIM
typedef __builtin_va_list va_list;
#define va_start(ap, last) __builtin_va_start(ap, last)
#define va_arg(ap, type) __builtin_va_arg(ap, type)
#define va_end(ap) __builtin_va_end(ap)
#define va_copy(dst, src) __builtin_va_copy(dst, src)
#endif
