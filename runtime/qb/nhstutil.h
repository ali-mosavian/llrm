/* String space and the string temporaries (QB rt/nhstutil.asm).

   String space is a run of entries from `str_first` to the boundary `str_end`.
   An entry is a header uword and the string's data, rounded up to a uword.  In
   use, the header is the address of the owning descriptor, which says how long
   the string is; free, it is the data's size plus one, so the two are told
   apart by the low bit.  A descriptor whose data is outside string space (a
   constant before it, a field buffer after) is not freed or moved. */
#ifndef QB_NHSTUTIL_H
#define QB_NHSTUTIL_H

#include "nheap.h"

/* The zero-length string's descriptor, which every empty result shares. */
extern SD str_nul;
/* The program level the temporaries are made at (b$curlevel). */
extern int cur_level;

void str_init(char *first, char *end);
void str_compact(void);

/* The heap trades room with string space. */
uword str_give_tail(void);
void str_take(uword bytes);

/* Gives `owner` a new string of `len` bytes and returns its data; out of room
   is Out of string space. */
char *str_alloc(SD *owner, uword len);
/* Frees `owner`'s string, leaving the descriptor as it was. */
void str_release(SD *owner);
/* `owner`'s descriptor is moving `delta` bytes (array relocation): its string
   follows. */
void str_owner_moved(SD *owner, int delta);
/* `to` takes over the string of the temporary `from`, which is freed as a
   descriptor. */
void str_adopt(SD *to, SD *from);

SD *str_tmp(uword len, char **data);
/* B$SASS's work (stcore.c) */
void str_assign(SD *source, SD *destination);
SD *str_tmp_copy(SD *source, uword from, uword len);
byte str_is_tmp(const SD *sd);
void str_tmp_free(SD *sd);
void str_all_tmp_free(int level);

#endif
