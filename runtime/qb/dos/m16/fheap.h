/* The far heap (QB rt/fhinit.asm, fhutil.asm): the memory above DGROUP up to
   the top of the program's DOS block, in paragraphs.  Its entries are the FHD
   at the head of each array descriptor, linked in descending address order
   between two fixed ones, the heap's top and its bottom. */
#ifndef QB_FHEAP_H
#define QB_FHEAP_H

#include "ad.h"

/* An FHD is the first part of an AD. */
typedef AD FHD;

/* Allocates `bytes`, zeroed, for `owner`; out of room is Out of memory. */
void fh_alloc(FHD *owner, unsigned long bytes);
void fh_free(FHD *owner);
unsigned long fh_free_bytes(void);

#endif
