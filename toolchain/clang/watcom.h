/* Open Watcom's headers on clang, force-included for the x86_*_wat targets, with -fdeclspec (`__declspec`).
   Its headers mark the library `__declspec(__watcall)`: the Watcom register convention, which clang has as
   `__regcall` (the backend reads it as Watcom's where the default is another, `llc.rs`). */
#ifdef _M_I86
#define __watcall __regcall
#define __WAT_DS___watcall __regcall
#define __WAT_DS__WCCALLBACK __regcall
#else /* flat: regcall is the default already */
#define __watcall
#define __WAT_DS___watcall
#define __WAT_DS__WCCALLBACK
#endif
#define __declspec(name) __WAT_DS_##name
#define __WAT_DS___aborts __attribute__((noreturn))
#define __WAT_DS_dllimport
typedef __builtin_va_list __va_list;
#define __int64 long long
