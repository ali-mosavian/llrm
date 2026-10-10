/* The target the portable QB runtime (runtime/qb) is built for: flat 32-bit DOS under DOS/32A.  One address
   space: a near pointer is a dword and nothing is far, so the names the real-mode target needs for its two
   kinds of pointer are the plain ones here.

   u8 u16 u32       fixed widths, for what the frontend's ABI lays out
   uword            a machine word: what a near pointer or the size of a heap entry is held in
   QB_FAR           nothing: every pointer reaches all of memory
   os_data          what the OS layer's calls take for data */
#ifndef QB_PLATFORM_H
#define QB_PLATFORM_H

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned long u32;
typedef unsigned int uword;

#define QB_FAR

/* The caller's variable, as a READ, GET or PUT is passed it. */
typedef void *qb_data_ptr;

typedef const char *os_path;
typedef const unsigned char *os_data;
typedef unsigned char *os_data_mut;

/* A near pointer as the data pointers the frontend passes and the OS layer takes. */
#define QB_FAR_OF(p) ((void *)(p))

/* The video memory at paragraph `segment`: the graphics and text screens. DOS/32A maps the first megabyte at
   linear 0. */
#define QB_VIDEO_MEMORY(segment) ((void *)((unsigned long)(segment) << 4))

/* The pointer to segment:offset, as a real-mode interrupt returns one. */
#define QB_REAL_POINTER(segment, offset) ((void *)(((unsigned long)(segment) << 4) + (offset)))

/* A function pointer in a startup table (xi.h, rtinit.h). */
typedef void (*qb_init_fn)(void);

/* The pointer a data pointer is. */
#define QB_NEAR_OF(p) ((void *)(p))

#endif
