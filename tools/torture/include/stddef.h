#ifndef _TORTURE_STDDEF_H
#define _TORTURE_STDDEF_H
#ifdef __386__
typedef unsigned size_t;
typedef int ptrdiff_t;
#else
typedef unsigned size_t;
typedef int ptrdiff_t;
#endif
typedef unsigned short wchar_t;
#define NULL 0
#define offsetof(type, member) ((size_t)&((type *)0)->member)
#endif
