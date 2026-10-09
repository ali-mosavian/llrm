/* The target the portable QB runtime (runtime/qb) is built for: real-mode DOS,
   16-bit.  A new target brings its own platform.h, and the platform sources
   beside it, and nothing in runtime/qb names a segment, a paragraph or a DOS
   call.

   u8 u16 u32       fixed widths, for what the frontend's ABI lays out uword a
   machine word: what a near pointer or the size of a heap entry is held in
   QB_FAR           a pointer past the program's near data, which is what the
   frontend passes for a caller's variable os_data          what the OS layer's
   calls take for data */
#ifndef QB_PLATFORM_H
#define QB_PLATFORM_H

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned long u32;
typedef unsigned int uword;

#define QB_FAR __far

/* The caller's variable, as a READ, GET or PUT is passed it. */
typedef void QB_FAR *qb_data_ptr;

typedef const char QB_FAR *os_path;
typedef const unsigned char QB_FAR *os_data;
typedef unsigned char QB_FAR *os_data_mut;

/* A near pointer as the data pointers the frontend passes and the OS layer
   takes. */
#define QB_FAR_OF(p) ((void QB_FAR *)(p))

/* A function pointer in a startup table (xi.h, rtinit.h). */
typedef void (QB_FAR *qb_init_fn)(void);

/* A near address of what a data pointer addresses, when it is in the program's
   near data. */
#define QB_NEAR_OF(p) ((void *)(uword)(unsigned long)(p))

#endif
