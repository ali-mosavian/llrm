/* The portable runtime on the host, for tests of the allocator: a word is as wide as a pointer. */
#ifndef QB_PLATFORM_H
#define QB_PLATFORM_H
#include <string.h>
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long uword;
#define QB_FAR
typedef void *qb_data_ptr;
typedef const char *os_path;
typedef const unsigned char *os_data;
typedef unsigned char *os_data_mut;
#define QB_FAR_OF(p) ((void *)(p))
typedef void (*qb_init_fn)(void);
#define QB_NEAR_OF(p) ((void *)(p))
#define QB_STRING_LIMIT 0x7FFFFFFFL
#endif
