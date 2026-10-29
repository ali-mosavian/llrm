/* Open Watcom's libc reads a long double as 8 bytes and a wchar_t as 2: the row compiles them so. */
#define LONG_DOUBLE_BYTES 8
#include "x86_16.c"
_Static_assert(sizeof(__WCHAR_TYPE__) == 2 && sizeof(L"a"[0]) == 2, "wchar_t");
