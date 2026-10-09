/* The dynamic region of DGROUP (QB rt/nhinit.asm, nhlhcore.asm, nhstutil.asm):
   from the end of the stack to the top of the group.  String space is at the
   bottom and the local heap at the top, and the boundary between them
   (`heap_low`) moves as either needs room.

   Local heap entries tile the heap from `heap_low` up to `heap_top`.  Each is a
   header, the data, and a copy of its size at its end, so the heap can be
   walked either way. */
#ifndef QB_NHEAP_H
#define QB_NHEAP_H

#include "qb.h"

enum LhType { LH_FREE = 1, LH_ARRAY, LH_FILE };

typedef struct LhEntry {
    word size;     /* the whole entry in bytes, even */
    byte type;     /* an LhType */
    byte file;     /* an FDB's channel */
    void *owner;   /* what points at the data, for the type's relocation hook */
} LhEntry;

extern char *heap_low;
extern char *heap_top;

/* A type's hook, called when compaction moves an entry's data `delta` bytes up.
   */
typedef void (*LhMoved)(void *data, int delta);
void lh_on_move(enum LhType type, LhMoved moved);

void nh_init(char *first, char *top);
void *lh_alloc(word bytes, enum LhType type, void *owner, byte file);
void lh_free(void *data);
void lh_compact(void);
LhEntry *lh_entry(void *data);
void *lh_data(LhEntry *entry);

/* The two heaps trade room (nhstutil.c is the other side): the free tail of
   string space becomes free heap, and free heap at the boundary becomes string
   space.  False when there is none to take. */
int lh_take_from_strings(void);
void lh_give_free_to_strings(void);

#endif
