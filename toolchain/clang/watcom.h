/* Open Watcom's headers on clang, force-included for the x86_*_wat targets, with -fdeclspec (`__declspec`).
   The calling convention is the target's (`-mabi=watcom`), so `__watcall` carries nothing more. */
#define __watcall
typedef __builtin_va_list __va_list;
#define __int64 long long
