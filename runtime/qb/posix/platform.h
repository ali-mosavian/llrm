/* The target the portable QB runtime (runtime/qb) is built for on a host: a flat address space, fixed-width
   integers, and nothing far. A host's `unsigned` is 32 bits at m32 and wider machines alike, so a width the
   runtime lays out is a fixed-width type here (u8 u16 u32), and a machine word is `uword`.

   The video memory and the real-mode pointers of DOS do not exist: code that names them (bload.c, the
   DEF SEG family) is not built for a host until the video-memory policy is chosen (#4669). */
#ifndef QB_PLATFORM_H
#define QB_PLATFORM_H

#include <stdint.h>

typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef uintptr_t uword;

#define QB_FAR

typedef void *qb_data_ptr;

typedef const char *os_path;
typedef const unsigned char *os_data;
typedef unsigned char *os_data_mut;

#define QB_FAR_OF(p) ((void *)(p))
#define QB_NEAR_OF(p) ((void *)(p))

void *B_REAL(uint64_t linear);
#define QB_REAL_POINTER(segment, offset) B_REAL(((uint64_t)(segment) << 4) + (offset))

#define QB_STRING_LIMIT 0x7FFFFFFFL

typedef void (*qb_init_fn)(void);

#endif
