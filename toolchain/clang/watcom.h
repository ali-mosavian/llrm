/* Open Watcom's headers on clang, force-included for the x86_*_wat targets, with -fdeclspec (`__declspec`).
   Its headers mark the library `__declspec(__watcall)`: the Watcom register convention. */
/* The Watcom register convention is the rows' default (-fdefault-calling-conv=regcall): a __watcall of the headers adds nothing, and on a
   variadic function (fprintf) __regcall is an error. */
#define __watcall
#define __WAT_DS___watcall
#define __WAT_DS__WCCALLBACK
#define __declspec(name) __WAT_DS_##name
#define __WAT_DS___aborts __attribute__((noreturn))
#define __WAT_DS_dllimport
typedef __builtin_va_list __va_list;
#define __int64 long long
