/* The C library's string and memory routines that the QB runtime names, for a wasm32 module with no libc.
   Bodies are in ../libc.c; the block routines lower to memory.copy and memory.fill. */
#ifndef LLRM_WASM_STRING_H
#define LLRM_WASM_STRING_H
#include <stddef.h>

void *memcpy(void *to, const void *from, size_t n);
void *memmove(void *to, const void *from, size_t n);
void *memset(void *to, int value, size_t n);
int memcmp(const void *a, const void *b, size_t n);
size_t strlen(const char *s);
int strcmp(const char *a, const char *b);
char *strcpy(char *to, const char *from);

#endif
