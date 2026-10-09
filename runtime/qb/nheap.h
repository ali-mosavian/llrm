/* The dynamic region of DGROUP (QB rt/nhlhcore.asm, nhinit.asm, nhstutil.asm): from qb_atopsp to the
   top of the group, string space grows up from the bottom and the local heap grows down from the
   top, and the boundary between them moves.  Entries are addressed by DGROUP offset.

   String space: entries of a header word then the data.  An allocated entry's header is the
   offset of the string's descriptor (even); a free one's is its data size plus one (odd).  The word
   at str_end is 0xFFFF.

   Local heap: entry pointers name the entry's type byte, its highest byte; the entry reaches down
   from there, its length is repeated at its lowest word, and the next entry is `length` below.  The
   END entry lies at heap_end, next to the string space. */
#ifndef QB_NHEAP_H
#define QB_NHEAP_H

#include "qb.h"

enum { LH_FREE = 0x01, LH_ARRAY = 0x02, LH_END = 0x04, LH_FILE = 0x08 };
enum { LH_STD_HDR = 6, LH_FDB_HDR = 10 };

#define LHTYPE(p) B(p)
#define LHFNUM(p) B((p) - 1)
#define LHLEN(p) W((p) - 3)
#define LHBAKP(p) W((p) - 5)

extern word nh_first, nh_last;
extern word str_first, str_free, str_end;
extern word heap_first, heap_free, heap_end;

/* nheap.c */
void nh_init(word first, word last);
word lh_alloc(word size, byte type, word owner);
void lh_free(word data);
void lh_compact(void);
word lh_entry(word data);
word lh_from_ss(void);
void lh_set_free(void);
void lh_adjust(word entry, word delta);

/* strcore.c */
void str_init(word first, word end);
void str_compact(void);
void str_set_free(void);
void str_from_lh(void);

#endif
