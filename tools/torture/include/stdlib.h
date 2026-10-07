#ifndef _TORTURE_STDLIB_H
#define _TORTURE_STDLIB_H
#include <stddef.h>
void exit(int);
void abort(void);
void *malloc(size_t);
void *calloc(size_t, size_t);
void *realloc(void *, size_t);
void free(void *);
int abs(int);
long labs(long);
#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1
#endif
